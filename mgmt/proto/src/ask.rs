// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A managed system's request carried as QSI encodes it. The web server
//! builds a family's request; `mgmtd` checks that the pages may send it
//! and that the system's hello offers it, sends it on the system's
//! session in the signed-on user's name, and hands the reply back.

use ipc::wire_struct;
use qsi::any::{AnyReply, AnyRequest};
use qsi::caps;

/// A request, by its family, operation and body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    /// Its family's number.
    pub family: u16,
    /// Its operation.
    pub op: u16,
    /// Its body, as QSI encodes it.
    pub body: Vec<u8>,
}
wire_struct!(Asked { family, op, body });

impl Asked {
    /// `req` as it is carried.
    pub fn of(req: &AnyRequest) -> Self {
        Self {
            family: req.family(),
            op: req.op(),
            body: req.encode(),
        }
    }

    /// The request again; `None` when it does not read.
    pub fn request(&self) -> Option<AnyRequest> {
        AnyRequest::decode(self.family, self.op, &self.body)
            .ok()
            .flatten()
    }
}

/// A reply, by its family, operation and body.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Answered {
    /// Its family's number.
    pub family: u16,
    /// Its operation.
    pub op: u16,
    /// Its body, as QSI encodes it.
    pub body: Vec<u8>,
}
wire_struct!(Answered { family, op, body });

impl Answered {
    /// `r` as it is carried.
    pub fn of(r: &AnyReply) -> Self {
        Self {
            family: r.family(),
            op: r.op(),
            body: r.encode(),
        }
    }

    /// The reply again; `None` when it does not read.
    pub fn reply(&self) -> Option<AnyReply> {
        AnyReply::decode(self.family, self.op, &self.body)
            .ok()
            .flatten()
    }
}

/// What the console says in its hello: every family and later version
/// whose requests its pages send or whose events it keeps.
pub const CONSOLE_CAPS: u64 = caps::HOST_V1
    | caps::SERVICING
    | caps::PHYSICAL_IO
    | caps::PARTITION_V2
    | caps::SWITCHES
    | caps::POOLS
    | caps::PARTITION_V3
    | caps::ACTIVATIONS
    | caps::POWER_ON
    | caps::PANEL_FUNCTIONS
    | caps::PROBLEM_NOTES
    | caps::DUMP_SPACE
    | caps::FIX_LEVEL;

/// Each capability, in an operator's words: what a system without it
/// does not offer.
const WORDS: &[(u64, &str)] = &[
    (caps::BASE, "QSI's base family"),
    (caps::PAIR, "pairing over QSI"),
    (caps::SERVICE, "its problems over QSI"),
    (caps::UPDATE, "code update over QSI"),
    (caps::SYSTEM_DUMPS, "its own dumps over QSI"),
    (caps::HOST, "a host's resources"),
    (caps::PARTITIONS, "partitions"),
    (
        caps::PARTITION,
        "partitions made and changed from a console",
    ),
    (caps::PROFILES, "partition profiles"),
    (caps::PANEL, "the control panel"),
    (caps::REFCODES, "partitions' reference codes"),
    (caps::DUMPS, "partitions' dumps"),
    (caps::CONSOLE, "the console"),
    (caps::VRES, "virtual resources"),
    (caps::PHYSICAL_IO, "physical I/O"),
    (
        caps::PARTITION_V2,
        "a running partition's entitlement changed",
    ),
    (
        caps::SWITCHES,
        "virtual switches made and changed from a console",
    ),
    (caps::POOLS, "storage pools made and changed from a console"),
    (caps::PARTITION_V3, "a partition's settings"),
    (caps::ACTIVATIONS, "activation times"),
    (caps::POWER_ON, "power-on settings set from a console"),
    (
        caps::PANEL_FUNCTIONS,
        "the list of its control panel functions",
    ),
    (caps::PROBLEM_NOTES, "notes on its problems"),
    (caps::DUMP_SPACE, "dump space"),
    (caps::FIX_LEVEL, "its fix level"),
];

/// What a system whose hello agreed `agreed` lacks for `need`, in
/// words: the first capability missing; `None` when it has them all.
pub fn lacks(agreed: u64, need: u64) -> Option<&'static str> {
    let missing = need & !agreed;
    if missing == 0 {
        return None;
    }
    Some(
        WORDS
            .iter()
            .find(|(bit, _)| missing & bit != 0)
            .map_or("a function of QSI", |(_, w)| *w),
    )
}

/// What a system lacks for `req`, as [`lacks`].
pub fn lacks_for(agreed: u64, req: &AnyRequest) -> Option<&'static str> {
    lacks(agreed, caps::of_request(req.family(), req.op()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use qsi::Empty;
    use qsi::host::{host, vres};

    #[test]
    fn a_request_and_a_reply_go_and_come_back_whole() {
        let req = AnyRequest::Vres(vres::Request::Pools(Empty));
        let back: Asked = ipc::decode(&ipc::encode(&Asked::of(&req))).unwrap();
        assert_eq!(back.request(), Some(req));
        let r = AnyReply::Vres(vres::Reply::Pools(Vec::new()));
        let back: Answered = ipc::decode(&ipc::encode(&Answered::of(&r))).unwrap();
        assert_eq!(back.reply(), Some(r));
    }

    #[test]
    fn what_an_older_host_lacks_is_named() {
        let set = AnyRequest::Host(host::Request::SetPowerOn(host::PowerOn {
            start_partitions: true,
            power_off: host::PowerOff::Delayed,
            delay: 600,
        }));
        assert_eq!(
            lacks_for(caps::HOST_V1, &set),
            Some("power-on settings set from a console")
        );
        assert_eq!(lacks_for(caps::HOST_V1 | caps::POWER_ON, &set), None);
        let pools = AnyRequest::Vres(vres::Request::Pools(Empty));
        assert_eq!(
            lacks_for(caps::STANDALONE, &pools),
            Some("virtual resources")
        );
        // Every capability bit this crate knows has its words.
        for bit in 0..28 {
            let b = 1u64 << bit;
            if (5..8).contains(&bit) {
                continue;
            }
            assert!(WORDS.iter().any(|(w, _)| *w == b), "bit {bit}");
        }
    }
}
