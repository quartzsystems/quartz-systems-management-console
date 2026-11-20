// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A host's power down: its partitions shut down first (or ended at
//! once), then off; a restart comes back after a pause, as a machine
//! would, and its sessions with it.

use std::thread;
use std::time::Duration;

use qsi::base::{PowerAction, PowerState};
use qsi::host::model::PartitionState;

use crate::system::{Shared, codes, lock};

/// How long a restarted system is away.
pub const AWAY: Duration = Duration::from_secs(3);

/// Begin `action`; the reply has been queued.
pub fn begin(sys: &Shared, action: PowerAction, immediate: bool) {
    let sys = sys.clone();
    thread::spawn(move || {
        // Let the reply go out first.
        thread::sleep(Duration::from_millis(300));
        {
            let mut s = lock(&sys);
            s.power = PowerState::Ending;
            let st = s.state();
            s.push(&qsi::any::AnyEvent::Base(qsi::base::Event::State(st)));
            let running: Vec<u32> = s
                .parts
                .iter()
                .filter(|p| p.status.state != PartitionState::NotActivated)
                .map(|p| p.definition.id)
                .collect();
            for id in running {
                if !immediate {
                    s.set_state(id, PartitionState::ShuttingDown);
                }
                codes::end(&mut s, id);
            }
            codes::post(&mut s, "D2001090", "Host powered down");
        }
        thread::sleep(Duration::from_millis(300));
        {
            let mut s = lock(&sys);
            s.power = PowerState::Off;
            s.up = false;
            s.generation += 1;
        }
        if action != PowerAction::Restart {
            return;
        }
        thread::sleep(AWAY);
        let mut s = lock(&sys);
        codes::post(&mut s, "C2001000", "Host IPL started");
        s.power = PowerState::Running;
        s.up = true;
        let st = s.state();
        s.push(&qsi::any::AnyEvent::Base(qsi::base::Event::State(st)));
    });
}
