// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Serving Shut down or restart console. OK asks PID 1 to restart or
//! power off at once; the message goes with it to the console's screen
//! and log, and onto every web page until the server stops. Nothing is
//! scheduled and nothing is connected before Phase 4.

use qsweb::http::Response;
use qsweb::msg::{Kind, Message};

use crate::app::routes::Rq;
use crate::shutdown::{Shutdown, page};
use crate::user_routes::outcome;

/// The longest message to signed-on users.
pub const MESSAGE_MAX: usize = 200;

fn view(rq: &Rq<'_>, messages: Vec<Message>) -> Shutdown {
    Shutdown {
        console: rq.app.console.name(),
        uptime: crate::pages::uptime(rq.app.console.uptime()),
        systems: "0".into(),
        terminals: "0".into(),
        scheduled: "0".into(),
        restart: true,
        message: String::new(),
        messages,
    }
}

/// `/console/shutdown`.
pub fn shutdown(rq: &Rq<'_>) -> Response {
    if !rq.post() {
        return rq.page(page(&rq.ctx, &view(rq, Vec::new())));
    }
    let f = rq.req.form();
    if f.get("ok").is_none() {
        return Response::redirect(&rq.ctx.url("/console"));
    }
    let restart = f.get("how") != Some("off");
    let text = f.get("m").unwrap_or("").trim().to_string();
    if text.chars().count() > MESSAGE_MAX || text.contains(['\n', '\r']) {
        let e = format!("The message is one line of at most {MESSAGE_MAX} characters.");
        let mut v = view(
            rq,
            vec![qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[&e])],
        );
        v.restart = restart;
        v.message = text;
        return rq.page(page(&rq.ctx, &v));
    }
    let user = rq.user().to_string();
    let by = if text.is_empty() {
        format!("{user} from the web interface")
    } else {
        format!("{user} from the web interface: {text}")
    };
    if !text.is_empty() {
        *rq.app
            .announced
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(qmcmsg::message(
            Kind::Info,
            &qmcmsg::ANNOUNCED,
            &[&user, &text],
        ));
    }
    let done = rq.app.console.power_down(restart, &by);
    rq.audit(
        if restart {
            "restart the console"
        } else {
            "power off the console"
        },
        outcome(&done),
    );
    let m = match done {
        Ok(()) if restart => qmcmsg::message(Kind::Info, &qmcmsg::RESTART_STARTED, &[]),
        Ok(()) => qmcmsg::message(Kind::Info, &qmcmsg::SHUTDOWN_STARTED, &[]),
        Err(e) => {
            *rq.app
                .announced
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[&e])
        }
    };
    // The answer is the page itself: the server may be gone before a
    // redirect is followed.
    rq.page(page(&rq.ctx, &view(rq, vec![m])))
}
