// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A second console beside the one under test, for suite 10: what
//! Phase 4's console does not do yet (a partition made, its profile,
//! its activation) done over QSI by the harness, paired with the host
//! as its own console. QS/400's session and pairing, through `mgmtd`'s
//! own `peer` module; nothing of QSI is written here.

use mgmtd::peer::link::{Link, hello, resolve};
use mgmtd::peer::pair::{ask, pair, refusal};
use qsi::any::{AnyReply, AnyRequest};
use qsi::base::Keylock;
use qsi::host::model::{
    Boot, ConsoleMode, IplSource, IplType, Lun, Memory, PartitionState, ProcessorMode, Processors,
    Profile, ScsiAdapter,
};
use qsi::host::{partition, partitions, profiles, refcodes};
use qsitrust::{Fingerprint, Identity};

/// The harness console's name, as the host lists it.
pub const NAME: &str = "MCHARNESS";
const USER: &str = "harness";

/// The harness, paired.
pub struct Beside {
    identity: Identity,
    pin: Fingerprint,
    address: String,
    port: u16,
}

impl Beside {
    /// Pair with the host at `address`:`port` with `password`.
    ///
    /// # Errors
    /// The pairing failed.
    pub fn pair(address: &str, port: u16, password: &str) -> Result<Self, String> {
        let identity = sim::fixture::identity(NAME, "", 0);
        let done = pair(
            address,
            port,
            &identity,
            hello(NAME, "V1R0M0"),
            password,
            USER,
        )
        .map_err(|r| format!("the harness's pairing: {} {}", r.main.msgid, r.main.text))?;
        Ok(Self {
            identity,
            pin: done.paired.fingerprint,
            address: address.into(),
            port,
        })
    }

    /// Ask the host `r` on a connection of its own; its reply, or its
    /// refusal as `msgid text`.
    ///
    /// # Errors
    /// No connection, no reply, or the host refused.
    pub fn ask(&self, r: &AnyRequest) -> Result<AnyReply, String> {
        let to = resolve(&self.address, self.port).map_err(|f| f.words())?;
        // A host three levels deep can be slow to say hello: try again.
        let mut tries = 0;
        let mut link = loop {
            match Link::connect(to, &self.identity, hello(NAME, "V1R0M0"), Some(self.pin)) {
                Ok(l) => break l,
                Err(f) if tries < 5 => {
                    tries += 1;
                    eprintln!("the harness's connection, try {tries}: {}", f.words());
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
                Err(f) => return Err(f.words()),
            }
        };
        let mut besides = Vec::new();
        let reply = ask(&mut link, USER, r, &mut besides);
        link.close();
        let reply = reply?;
        if let Some(no) = refusal(&reply) {
            return Err(format!("{} {}", no.main.msgid, no.main.text));
        }
        reply
            .decode()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "a reply that does not read".to_string())
    }

    /// A QS/400 partition of `name` on the host's first further disk,
    /// its profile `NORMAL` the default: its ID.
    ///
    /// # Errors
    /// The host refused a step.
    pub fn partition(&self, name: &str) -> Result<u32, String> {
        let AnyReply::Partition(partition::Reply::Create(d)) = self.ask(&AnyRequest::Partition(
            partition::Request::Create(partition::Create { name: name.into() }),
        ))?
        else {
            return Err("create: another reply".into());
        };
        self.ask(&AnyRequest::Profiles(profiles::Request::Put(
            profiles::Put {
                id: d.id,
                profile: profile(),
                revision: None,
            },
        )))?;
        self.ask(&AnyRequest::Profiles(profiles::Request::SetDefault(
            profiles::SetDefault {
                id: d.id,
                name: "NORMAL".into(),
            },
        )))?;
        Ok(d.id)
    }

    /// Activate `id` with its default profile, the keylock `keylock`
    /// for this activation when given.
    ///
    /// # Errors
    /// The host refused.
    pub fn activate(&self, id: u32, keylock: Option<Keylock>) -> Result<(), String> {
        self.ask(&AnyRequest::Partition(partition::Request::Activate(
            partition::Activate {
                id,
                profile: String::new(),
                source: None,
                keylock,
            },
        )))
        .map(|_| ())
    }

    /// End `id` at once.
    ///
    /// # Errors
    /// The host refused.
    pub fn end(&self, id: u32) -> Result<(), String> {
        self.ask(&AnyRequest::Partition(partition::Request::ShutDown(
            partition::ShutDown {
                id,
                kind: partition::ShutdownKind::Immediate,
                delay: 0,
                restart: false,
            },
        )))
        .map(|_| ())
    }

    /// The newest page of `id`'s history at the host: seq and code.
    ///
    /// # Errors
    /// The host refused.
    pub fn history(&self, id: u32) -> Result<Vec<(u64, String)>, String> {
        let AnyReply::Refcodes(refcodes::Reply::History(page)) = self.ask(
            &AnyRequest::Refcodes(refcodes::Request::History(refcodes::History {
                id,
                before: None,
                limit: 50,
            })),
        )?
        else {
            return Err("history: another reply".into());
        };
        Ok(page
            .codes
            .into_iter()
            .map(|c| (c.refcode.seq, c.refcode.code))
            .collect())
    }

    /// The state of `id`.
    ///
    /// # Errors
    /// The host refused, or has no such partition.
    pub fn state(&self, id: u32) -> Result<PartitionState, String> {
        let AnyReply::Partitions(partitions::Reply::List(all)) = self.ask(
            &AnyRequest::Partitions(partitions::Request::List(qsi::message::Empty)),
        )?
        else {
            return Err("list: another reply".into());
        };
        all.iter()
            .find(|s| s.id == id)
            .map(|s| s.state)
            .ok_or_else(|| format!("no partition {id}"))
    }
}

/// QS/400 as QS/VS's own suites give it: one dedicated processor, two
/// gigabytes, its recorded side, the console on the console port, its
/// disk the host's `/dev/vdb`.
fn profile() -> Profile {
    Profile {
        name: "NORMAL".into(),
        processors: Processors {
            mode: ProcessorMode::Dedicated,
            minimum: 1,
            desired: 1,
            maximum: 1,
            minimum_units: None,
            desired_units: None,
            maximum_units: None,
            sharing: None,
            weight: None,
            pool: None,
        },
        memory: Memory {
            minimum_mib: 2048,
            desired_mib: 2048,
            maximum_mib: 2048,
        },
        boot: Boot {
            source: IplSource::Recorded,
            ipl_type: IplType::Unattended,
            console: ConsoleMode::Partition,
        },
        scsi: vec![ScsiAdapter {
            slot: 2,
            luns: vec![Lun {
                lun: 0,
                backing: "/dev/vdb".into(),
                serial: None,
            }],
        }],
        ethernet: Vec::new(),
        optical: Vec::new(),
        tape: Vec::new(),
        physical_io: Vec::new(),
    }
}
