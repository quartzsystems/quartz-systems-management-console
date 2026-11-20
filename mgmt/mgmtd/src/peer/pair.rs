// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Pairing with a managed system: QS/400's exchange (`qsitrust::pair`)
//! over a connection that accepts any certificate, with the one-time
//! password its operator read there. The password is used here once and
//! never kept, logged or sent: SPAKE2 proves it.

use std::time::{Duration, Instant};

use mgmtproto::{Note, Refusal};
use qsi::any::AnyRequest;
use qsi::{Hello, pair};
use qsisess::{Incoming, Outcome, Reply};
use qsitrust::Identity;
use qsitrust::pair::Paired;

use super::link::{Failed, Link, resolve};
use crate::words::qmc;

/// How long a request waits for its reply.
pub const WAIT: Duration = Duration::from_secs(30);

/// A refusal as the managed system gave it, every message.
pub fn refusal(reply: &Reply) -> Option<Refusal> {
    match &reply.outcome {
        Outcome::Refused {
            msgid,
            text,
            help,
            also,
        } => Some(Refusal {
            main: Note {
                msgid: msgid.clone(),
                text: text.clone(),
                help: help.clone(),
            },
            also: also
                .iter()
                .map(|n| Note {
                    msgid: n.msgid.clone(),
                    text: n.text.clone(),
                    help: n.help.clone(),
                })
                .collect(),
        }),
        Outcome::Result { .. } => None,
    }
}

/// Ask on `link` as `user` and wait for the reply; what else comes is
/// put in `besides`.
///
/// # Errors
/// The session would not send it, ended, or no reply came in time.
pub fn ask(
    link: &mut Link,
    user: &str,
    r: &AnyRequest,
    besides: &mut Vec<Incoming>,
) -> Result<Reply, String> {
    let id = link
        .session
        .request_any(user, r)
        .map_err(|e| e.to_string())?;
    let until = Instant::now() + WAIT;
    while Instant::now() < until {
        for i in link.step()? {
            match i {
                Incoming::Reply(rep) if rep.id == id => return Ok(rep),
                other => besides.push(other),
            }
        }
    }
    Err("no reply in time".into())
}

/// The connection's failure as the console's message.
pub fn failed(at: &str, f: &Failed) -> Refusal {
    match f {
        Failed::Unreachable(w) => qmc(&qmcmsg::NO_ANSWER, &[at, w]),
        Failed::TurnedAway(_) => qmc(&qmcmsg::TURNED_AWAY, &[at]),
        Failed::Changed(_) => qmc(&qmcmsg::CERTIFICATE_CHANGED, &[at]),
        Failed::NotQsi(w) => qmc(&qmcmsg::NOT_QSI, &[at, w]),
    }
}

/// What a pairing gives: the system's pin and name, and its hello.
#[derive(Debug)]
pub struct Done {
    /// Its certificate and the name it gave.
    pub paired: Paired,
    /// Its hello.
    pub hello: Hello,
}

/// Pair with the system at `address`:`port` as `identity` (whose hello
/// is `me`), with `password`, for `user`.
///
/// # Errors
/// The connection's failure, the system's refusal as it gave it, or the
/// console's message when the system's confirmation does not hold.
pub fn pair(
    address: &str,
    port: u16,
    identity: &Identity,
    me: Hello,
    password: &str,
    user: &str,
) -> Result<Done, Refusal> {
    let at = format!("{address}:{port}");
    let to = resolve(address, port).map_err(|f| failed(&at, &f))?;
    let name = me.name.clone();
    let mut link = Link::connect(to, identity, me, None).map_err(|f| failed(&at, &f))?;
    let done = exchange(&mut link, identity, password, &name, user, &at);
    let hello = link.hello.clone();
    link.close();
    done.map(|paired| Done { paired, hello })
}

fn exchange(
    link: &mut Link,
    identity: &Identity,
    password: &str,
    name: &str,
    user: &str,
    at: &str,
) -> Result<Paired, Refusal> {
    let gone = |e: String| qmc(&qmcmsg::NO_ANSWER, &[at, &e]);
    let mut besides = Vec::new();
    let binding = link.binding(identity).map_err(gone)?;
    let (console, start) = qsitrust::pair::Console::start(password, binding, name)
        .map_err(|e| qmc(&qmcmsg::NOT_QSI, &[at, &e.to_string()]))?;
    let reply = ask(
        link,
        user,
        &AnyRequest::Pair(pair::Request::Start(pair::Start {
            spake: start.spake,
            name: start.name,
        })),
        &mut besides,
    )
    .map_err(gone)?;
    if let Some(r) = refusal(&reply) {
        return Err(r);
    }
    let Ok(Some(pair::Reply::Answer(a))) = reply.typed::<pair::Reply>() else {
        return Err(qmc(
            &qmcmsg::NOT_QSI,
            &[at, "its pairing answer does not read"],
        ));
    };
    let answered = console.answer(&qsitrust::pair::Answer {
        spake: a.spake,
        confirm: a.confirm,
        name: a.name,
    });
    let Ok((confirm, paired)) = answered else {
        // A wrong password, or not that system: confirm nothing, so the
        // system counts the attempt and says so.
        let r = ask(
            link,
            user,
            &AnyRequest::Pair(pair::Request::Confirm(pair::Confirm {
                confirm: vec![0; 32],
            })),
            &mut besides,
        )
        .map_err(gone)?;
        let mut out = qmc(&qmcmsg::NOT_CONFIRMED, &[at]);
        if let Some(theirs) = refusal(&r) {
            out.also = theirs.notes().into_iter().cloned().collect();
        }
        return Err(out);
    };
    let done = ask(
        link,
        user,
        &AnyRequest::Pair(pair::Request::Confirm(pair::Confirm {
            confirm: confirm.confirm,
        })),
        &mut besides,
    )
    .map_err(gone)?;
    if let Some(r) = refusal(&done) {
        return Err(r);
    }
    Ok(paired)
}
