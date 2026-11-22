// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What the system answers to each request a pinned console sends, in
//! the words a QS/VS host uses. Pairing and the streams are the
//! connection's (`net`); this is everything else.

use qsi::any::{AnyEvent, AnyReply, AnyRequest};
use qsi::base::{self, Keylock, PowerAction, PowerState};
use qsi::host::model::{IplSource, IplType, PartitionState};
use qsi::host::partitions::{Activation, Adapter};
use qsi::host::{console, host, panel, partitions, refcodes, vres};
use qsi::{Empty, PeerKind};

use super::refuse::{self, Refusal};
use super::{System, codes};

/// What a request comes to.
#[derive(Debug)]
#[allow(clippy::large_enum_variant, reason = "made once and passed on")]
pub enum Answer {
    /// A reply.
    Reply(AnyReply),
    /// A refusal: the message first, then the others that apply.
    Refuse(Vec<(String, String)>),
    /// A subscription from the event's number given.
    Subscribe(Option<u64>),
    /// A console to attach: the partition and whether to take it over.
    Attach(u32, bool),
    /// An image's upload to take: its name and size.
    Upload(String, u64),
    /// A stage of an update image: its stream.
    Stage(String, u64),
    /// A dump's download: partition (0 for the system's own), name, size.
    Download(u32, String, u64),
    /// An activation into the pending slot.
    Activate(String),
    /// An operation this system does not answer.
    Unknown,
    /// A power down begun: the connection's controller carries it out.
    Power(PowerAction, bool),
}

fn refuse(id: &str, text: &str) -> Answer {
    Answer::Refuse(vec![(id.into(), text.into())])
}

/// A family's answer, or its refusal.
fn of(r: Result<AnyReply, Refusal>) -> Answer {
    match r {
        Ok(rep) => Answer::Reply(rep),
        Err(notes) => Answer::Refuse(notes),
    }
}

/// A partition named by ID that is not there.
fn no_partition(id: u32) -> Answer {
    Answer::Refuse(refuse::no_partition(id))
}

/// The refusal a test asked for of `req`'s family, if any.
fn told(s: &System, req: &AnyRequest) -> Option<Answer> {
    let (f, op) = (req.family(), req.op());
    s.faults
        .refuse_family
        .iter()
        .find(|r| r.family == f && r.op.is_none_or(|o| o == op))
        .map(|r| refuse(&r.id, &r.text))
}

/// Answer `r`, asked by `user`.
#[allow(clippy::too_many_lines, reason = "one arm a request")]
pub fn answer(s: &mut System, req: &AnyRequest, user: &str) -> Answer {
    s.note(format!(
        "request {:04X}/{:04X} from user {user}",
        req.family(),
        req.op()
    ));
    if let Some(a) = told(s, req) {
        return a;
    }
    let host = s.hello.kind == PeerKind::Host;
    if let AnyRequest::Base(b) = req
        && let Some(r) = s.own_dump_answer(b, user)
    {
        return of(r);
    }
    match req {
        AnyRequest::Base(base::Request::DownloadDump(d)) => match s.own_dump(&d.name) {
            Ok(n) => Answer::Download(0, d.name.clone(), n),
            Err(r) => Answer::Refuse(r),
        },
        AnyRequest::Service(r) => of(s.service_answer(r, user)),
        AnyRequest::Dumps(qsi::host::dumps::Request::Download(d)) if host => {
            match s.part_dump(d.id, &d.name) {
                Ok(n) => Answer::Download(d.id, d.name.clone(), n),
                Err(r) => Answer::Refuse(r),
            }
        }
        AnyRequest::Dumps(r) if host => of(s.dumps_answer(r)),
        AnyRequest::Update(qsi::update::Request::Stage(st)) if host => {
            if s.servicing.staged.is_some() {
                return refuse(
                    "QSI1107",
                    "An update image is staged already: discard it first.",
                );
            }
            Answer::Stage(st.name.clone(), st.bytes)
        }
        AnyRequest::Update(qsi::update::Request::Activate(a)) if host => match s.may_activate(a) {
            Ok(slot) => Answer::Activate(slot),
            Err(r) => Answer::Refuse(r),
        },
        AnyRequest::Update(r) if host => of(s.update_answer(r)),
        AnyRequest::Base(b) => match b {
            base::Request::Identify(_) => {
                Answer::Reply(AnyReply::Base(base::Reply::Identify(s.identify())))
            }
            base::Request::State(_) => Answer::Reply(AnyReply::Base(base::Reply::State(s.state()))),
            base::Request::Refcodes(q) => {
                let limit = usize::try_from(q.limit.clamp(1, 500)).unwrap_or(500);
                let mut codes: Vec<_> = s
                    .codes
                    .iter()
                    .rev()
                    .filter(|c| q.before.is_none_or(|b| c.seq < b))
                    .take(limit + 1)
                    .cloned()
                    .collect();
                let more = (codes.len() > limit).then(|| codes[limit - 1].seq);
                codes.truncate(limit);
                Answer::Reply(AnyReply::Base(base::Reply::Refcodes(base::RefcodeList {
                    codes,
                    more,
                })))
            }
            base::Request::Subscribe(q) => Answer::Subscribe(q.since),
            base::Request::Unsubscribe(_) => {
                Answer::Reply(AnyReply::Base(base::Reply::Unsubscribe(Empty)))
            }
            base::Request::Time(_) => {
                Answer::Reply(AnyReply::Base(base::Reply::Time(base::Time {
                    unix_ns: i64::try_from(super::now_ms()).unwrap_or(0) * 1_000_000,
                    offset_s: 0,
                    zone: "UTC".into(),
                })))
            }
            base::Request::Power(p) => {
                if let Some((id, text)) = s.faults.refuse_power.clone() {
                    return refuse(&id, &text);
                }
                if s.power != PowerState::Running {
                    return refuse(
                        "QVS1010",
                        &format!("The host is {:?}: it cannot be powered down now.", s.power),
                    );
                }
                Answer::Power(p.action, p.immediate)
            }
            _ => Answer::Unknown,
        },
        AnyRequest::Host(h) if host => match h {
            host::Request::Host(_) => match &s.host {
                Some(x) => Answer::Reply(AnyReply::Host(host::Reply::Host(x.clone()))),
                None => refuse("QSI0301", "The partition manager did not answer."),
            },
            host::Request::Settings(_) => {
                Answer::Reply(AnyReply::Host(host::Reply::Settings(s.settings.clone())))
            }
            host::Request::Hardware(_) => match &s.hardware {
                Some(x) => Answer::Reply(AnyReply::Host(host::Reply::Hardware(x.clone()))),
                None => refuse("QSI0301", "The partition manager did not answer."),
            },
            host::Request::Devices(_) => {
                Answer::Reply(AnyReply::Host(host::Reply::Devices(s.devices.clone())))
            }
            host::Request::PowerOn(_) => {
                Answer::Reply(AnyReply::Host(host::Reply::PowerOn(s.power_on)))
            }
            host::Request::SetPowerOn(p) => {
                if p.delay > host::MAX_DELAY {
                    return refuse(
                        "QVS1022",
                        "The delay of a power off is 99999 seconds at most.",
                    );
                }
                if matches!(p.power_off, host::PowerOff::Other(_)) {
                    return Answer::Refuse(refuse::no_such("power off"));
                }
                s.power_on = *p;
                s.push(&AnyEvent::Host(host::Event::PowerOn(
                    host::PowerOnChanged {
                        power_on: *p,
                        by: user.into(),
                        time: super::now_s(),
                    },
                )));
                Answer::Reply(AnyReply::Host(host::Reply::SetPowerOn(Empty)))
            }
        },
        AnyRequest::Partitions(p) if host => match p {
            partitions::Request::List(_) => {
                Answer::Reply(AnyReply::Partitions(partitions::Reply::List(s.statuses())))
            }
            partitions::Request::Get(g) => match s.part(g.id) {
                Some(p) => Answer::Reply(AnyReply::Partitions(partitions::Reply::Get(
                    partitions::Partition {
                        definition: p.definition.clone(),
                        status: p.status.clone(),
                    },
                ))),
                None => no_partition(g.id),
            },
            partitions::Request::Adapters(a) => {
                if a.id != 0 && s.part_ref(a.id).is_none() {
                    return no_partition(a.id);
                }
                Answer::Reply(AnyReply::Partitions(partitions::Reply::Adapters(adapters(
                    s, a.id,
                ))))
            }
            partitions::Request::Activations(_) => {
                Answer::Reply(AnyReply::Partitions(partitions::Reply::Activations(
                    s.parts
                        .iter()
                        .map(|p| Activation {
                            id: p.definition.id,
                            since: p.since.filter(|_| p.active()),
                            profile: p.status.profile.clone(),
                        })
                        .collect(),
                )))
            }
        },
        AnyRequest::Partition(r) if host => {
            of(super::partition::answer(s, r, user).map(AnyReply::Partition))
        }
        AnyRequest::Profiles(r) if host => {
            of(super::profiles::answer(s, r).map(AnyReply::Profiles))
        }
        AnyRequest::Vres(vres::Request::ImageUpload(u)) if host => {
            match super::vres::media::upload_check(s, u, &s.upload.busy) {
                Ok(()) => Answer::Upload(u.name.clone(), u.bytes),
                Err(n) => Answer::Refuse(n),
            }
        }
        AnyRequest::Vres(r) if host => of(super::vres::answer(s, r).map(AnyReply::Vres)),
        AnyRequest::Refcodes(refcodes::Request::History(q)) if host => {
            let Some(p) = s.part(q.id) else {
                return no_partition(q.id);
            };
            let limit = usize::try_from(q.limit.clamp(1, 256)).unwrap_or(256);
            let mut codes: Vec<_> = p
                .history
                .iter()
                .rev()
                .filter(|c| q.before.is_none_or(|b| c.refcode.seq < b))
                .take(limit + 1)
                .cloned()
                .collect();
            let more = (codes.len() > limit).then(|| codes[limit - 1].refcode.seq);
            codes.truncate(limit);
            Answer::Reply(AnyReply::Refcodes(refcodes::Reply::History(
                refcodes::Page { codes, more },
            )))
        }
        AnyRequest::Panel(panel::Request::Function(f)) if host => function(s, f),
        AnyRequest::Panel(panel::Request::Function(f)) => own_function(s, f),
        AnyRequest::Panel(panel::Request::Functions(q)) => functions(s, q.id),
        AnyRequest::Console(cq) if elsewhere(host, cq).is_some() => {
            elsewhere(host, cq).unwrap_or(Answer::Unknown)
        }
        AnyRequest::Console(cq) => match cq {
            console::Request::Attach(at) => match s.part(at.id) {
                None => no_partition(at.id),
                Some(p) if p.status.state == PartitionState::NotActivated => refuse(
                    "QVS2003",
                    &format!("Partition {} is not activated.", p.status.name),
                ),
                Some(_) => Answer::Attach(at.id, at.take_over),
            },
            console::Request::Screen(sq) => {
                let Some(p) = s.part(sq.id) else {
                    return no_partition(sq.id);
                };
                let frame = p.screen.as_ref().map(|r| r.frame.clone());
                let text = frame.as_ref().map_or_else(String::new, |f| {
                    f.cells
                        .chunks(usize::from(f.cols).max(1))
                        .map(|row| row.iter().map(|c| c.ch).collect::<String>())
                        .collect::<Vec<_>>()
                        .join("\n")
                });
                Answer::Reply(AnyReply::Console(console::Reply::Screen(
                    console::Capture {
                        serial: false,
                        frame,
                        text,
                    },
                )))
            }
        },
        _ => Answer::Unknown,
    }
}

/// The virtual Ethernet adapters of partition `id`'s profiles, or of
/// every partition's for 0.
fn adapters(s: &System, id: u32) -> Vec<Adapter> {
    let mut v = Vec::new();
    for p in s.parts.iter().filter(|p| id == 0 || p.definition.id == id) {
        for k in &p.profiles {
            for e in &k.profile.ethernet {
                v.push(Adapter {
                    partition: p.definition.id,
                    name: p.status.name.clone(),
                    profile: k.profile.name.clone(),
                    slot: e.slot,
                    switch: e.switch.clone().unwrap_or_default(),
                    vlan: u16::try_from(e.vlan).unwrap_or(0),
                    tagged: e
                        .tagged
                        .iter()
                        .filter_map(|t| u16::try_from(*t).ok())
                        .collect(),
                    mac: e.mac.clone().unwrap_or_default(),
                    required: e.required,
                });
            }
        }
    }
    v
}

/// The refusal of a console request whose ID this system does not
/// answer: a host answers its partitions, a standalone system its own
/// console at ID 0.
fn elsewhere(host: bool, cq: &console::Request) -> Option<Answer> {
    let id = match cq {
        console::Request::Attach(a) => a.id,
        console::Request::Screen(q) => q.id,
    };
    match (host, id) {
        (true, 0) => Some(no_partition(0)),
        (false, n) if n != 0 => Some(refuse(
            "QSI0331",
            &format!("No partition {n}: this system answers its own console at ID 0."),
        )),
        _ => None,
    }
}

/// The functions the system lists for partition `id`: a host's are the
/// fixture's, a standalone system's its own at ID 0.
fn functions(s: &mut System, id: u32) -> Answer {
    use crate::fixture::{FUNCTIONS, STANDALONE_FUNCTIONS};
    let (list, manual, confirm): (&[(u8, &str)], &[u8], &[u8]) = if s.is_host() {
        if id == 0 || s.part(id).is_none() {
            return no_partition(id);
        }
        (&FUNCTIONS, &[3, 8, 21, 22, 34], &[8, 22])
    } else {
        if id != 0 {
            return refuse("QSI0331", &format!("No partition {id}."));
        }
        (&STANDALONE_FUNCTIONS, &[3, 8, 21, 22], &[3, 8, 22])
    };
    let list = list
        .iter()
        .map(|(n, text)| panel::FunctionInfo {
            function: *n,
            text: (*text).into(),
            manual: manual.contains(n),
            confirm: confirm.contains(n),
        })
        .collect();
    Answer::Reply(AnyReply::Panel(panel::Reply::Functions(list)))
}

/// A standalone system's own function at ID 0, with QSI's refusals.
fn own_function(s: &mut System, f: &panel::Function) -> Answer {
    if let Some((id, text)) = s.faults.refuse_panel.clone() {
        return refuse(&id, &text);
    }
    if f.id != 0 {
        return refuse("QSI0331", &format!("No partition {}.", f.id));
    }
    let n = f.function;
    if !matches!(n, 1 | 3 | 8 | 11..=22) {
        return refuse("QSI0451", &format!("This system has no function {n:02}."));
    }
    if matches!(n, 3 | 8 | 21 | 22) {
        if s.keylock != Some(Keylock::Manual) {
            return refuse(
                "QSI0452",
                &format!("Function {n:02} needs the keylock in manual."),
            );
        }
        if s.power != PowerState::Running {
            return refuse(
                "QSI0453",
                &format!("Function {n:02} waits for the IPL to complete."),
            );
        }
    }
    let code = s.current().map_or_else(|| "00000000".into(), |r| r.code);
    let text = match n {
        1 => format!(
            "01  IPL source {}, keylock {}",
            s.slot,
            format!("{:?}", s.keylock.unwrap_or(Keylock::Normal)).to_lowercase()
        ),
        11 => format!("11  {code}"),
        12..=19 => format!("{n:02}  00000000"),
        20 => format!("20  type {}, serial number {}", s.model, s.hello.serial),
        3 => "03  IPL started".into(),
        8 => "08  system powered off".into(),
        21 => "21  dedicated service tools shown on the console".into(),
        _ => "22  dump begun".into(),
    };
    Answer::Reply(AnyReply::Panel(panel::Reply::Function(panel::Shown {
        text,
    })))
}

/// A control panel function, as `docs/PANEL.md` of QS/VS has them.
fn function(s: &mut System, f: &panel::Function) -> Answer {
    if let Some((id, text)) = s.faults.refuse_panel.clone() {
        return refuse(&id, &text);
    }
    let Some(p) = s.part(f.id) else {
        return no_partition(f.id);
    };
    let name = p.status.name.clone();
    if !p.settings.panel {
        return refuse(
            "QVS2318",
            &format!(
                "Partition {name} does not allow control panel functions from a management console."
            ),
        );
    }
    let keylock = p.keylock();
    let running = p.status.state != PartitionState::NotActivated;
    let manual = keylock == Keylock::Manual;
    let needs_manual = matches!(f.function, 3 | 8 | 21 | 22 | 34);
    let needs_running = matches!(f.function, 3 | 8 | 21 | 22 | 34);
    if !matches!(f.function, 1..=3 | 8 | 11..=22 | 34) {
        return refuse(
            "QVS2306",
            &format!(
                "Function {:02} is not a control panel function.",
                f.function
            ),
        );
    }
    if needs_running && !running {
        return refuse("QVS2003", &format!("Partition {name} is not activated."));
    }
    if needs_manual && !manual {
        return refuse(
            "QVS2307",
            &format!(
                "Function {:02} needs the keylock in manual; it is {}.",
                f.function,
                format!("{keylock:?}").to_lowercase()
            ),
        );
    }
    let code = p.status.code.clone();
    let text = match f.function {
        1 => format!(
            "01  IPL source {}, IPL type {}, keylock {}",
            word_source(f.args.source.unwrap_or(IplSource::A)),
            word_type(f.args.ipl_type.unwrap_or(IplType::Unattended)),
            format!("{keylock:?}").to_lowercase()
        ),
        2 => {
            if let Some(k) = f.args.keylock
                && let Some(p) = s.part(f.id)
            {
                p.definition.keylock = k;
            }
            "02  next activation selected".into()
        }
        11 => format!("11  {code}"),
        12..=19 => format!("{:02}  00000000", f.function),
        20 => format!(
            "20  type QS/400 partition, serial number {}, host QS/VS V1R0M0, system V1R2M0",
            p_serial(s, f.id)
        ),
        3 => {
            codes::restart(s, f.id);
            "03  IPL restarted".into()
        }
        8 => {
            codes::end(s, f.id);
            "08  partition powered off".into()
        }
        21 => "21  dedicated service tools shown on the console".into(),
        22 => {
            codes::dump(s, f.id);
            format!("22  dump {}-1 begun", name.to_lowercase())
        }
        _ => {
            return refuse(
                "QVS2309",
                &format!("Partition {name} is not held: there is no memory to dump again."),
            );
        }
    };
    Answer::Reply(AnyReply::Panel(panel::Reply::Function(panel::Shown {
        text,
    })))
}

fn p_serial(s: &mut System, id: u32) -> String {
    s.part(id)
        .map(|p| p.definition.serial.clone())
        .unwrap_or_default()
}

fn word_source(s: IplSource) -> &'static str {
    match s {
        IplSource::B => "B",
        IplSource::D => "D",
        IplSource::Recorded => "recorded",
        _ => "A",
    }
}

fn word_type(t: IplType) -> &'static str {
    match t {
        IplType::Attended => "attended",
        _ => "unattended",
    }
}
