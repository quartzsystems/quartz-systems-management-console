// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 5: every destructive action does nothing by its confirmation's
//! GET, nothing by a POST without the form's token or with Cancel, and
//! is done by the confirmation's OK.

use std::time::Duration;

use qsi::host::model::PartitionState;

use super::{Lab, VS, expect, host, kept, lab, line_of, post_form};
use crate::suite::mgmt::{Fleet, page, until};
use crate::suite::web::{message_of, submit};

/// One destructive action.
struct Case {
    /// What it is.
    what: &'static str,
    /// Its confirmation page.
    path: String,
    /// What its OK posts, the token aside.
    fields: Vec<(&'static str, String)>,
    /// The message it is done with.
    done: &'static str,
    /// What of the host it changes.
    probe: fn(&Fleet) -> String,
}

fn parts(f: &Fleet) -> String {
    host(f)
        .parts
        .iter()
        .map(|p| format!("{} {:?}", p.definition.id, p.status.state))
        .collect::<Vec<_>>()
        .join(", ")
}

fn profiles(f: &Fleet) -> String {
    kept(f, "profiles 1")
}

fn vdisks(f: &Fleet) -> String {
    kept(f, "vdisks")
}

fn switches(f: &Fleet) -> String {
    kept(f, "switches")
}

fn images(f: &Fleet) -> String {
    kept(f, "images")
}

fn volumes(f: &Fleet) -> String {
    kept(f, "volumes")
}

fn procpools(f: &Fleet) -> String {
    kept(f, "procpools")
}

fn ms(fields: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    let mut v = vec![("ms", VS.to_string())];
    v.extend(fields.iter().map(|(k, x)| (*k, (*x).to_string())));
    v
}

fn shutdown(id: u32, how: &str, restart: bool) -> (String, Vec<(&'static str, String)>) {
    let mut q = format!("how={how}&delay=300");
    let mut f = vec![
        ("how", how.to_string()),
        ("delay", "300".to_string()),
        ("confirmed", "1".to_string()),
    ];
    if restart {
        q.push_str("&restart=on");
        f.push(("restart", "on".into()));
    }
    (
        format!("/qsmc/partition/{VS}/{id}/shutdown?confirm=1&{q}"),
        f,
    )
}

fn cases() -> Vec<Case> {
    let (p3, f3) = shutdown(3, "delayed", false);
    let (r1, g1) = shutdown(1, "delayed", true);
    let (i1, h1) = shutdown(1, "imm", false);
    vec![
        Case {
            what: "shut down QSPROD01 (delayed)",
            path: p3,
            fields: f3,
            done: "QMC3102",
            probe: parts,
        },
        Case {
            what: "restart QSDEV01",
            path: r1,
            fields: g1,
            done: "QMC3103",
            probe: |f| {
                host(f)
                    .lines
                    .iter()
                    .filter(|l| l.contains("request 0012/"))
                    .filter(|l| l.ends_with("from user admin"))
                    .count()
                    .to_string()
            },
        },
        Case {
            what: "shut down QSDEV01 (immediate)",
            path: i1,
            fields: h1,
            done: "QMC3102",
            probe: parts,
        },
        Case {
            what: "delete profile big-test of QSDEV01",
            path: format!("/qsmc/confirm/profile-delete?ms={VS}&p=1&n=big-test"),
            fields: ms(&[("p", "1"), ("n", "big-test")]),
            done: "QMC3205",
            probe: profiles,
        },
        Case {
            what: "delete partition QSTEST01 (its disks kept)",
            path: format!("/qsmc/confirm/partition-delete?ms={VS}&p=2"),
            fields: ms(&[("p", "2")]),
            done: "QMC3133",
            probe: parts,
        },
        Case {
            what: "delete partition QSDEV01 with its disks",
            path: format!("/qsmc/confirm/partition-delete?ms={VS}&p=1"),
            fields: ms(&[("p", "1"), ("disks", "on")]),
            done: "QMC3133",
            probe: vdisks,
        },
        Case {
            what: "delete virtual disk VD06",
            path: format!("/qsmc/confirm/vdisk-delete?ms={VS}&n=VD06&p=POOL1"),
            fields: ms(&[("n", "VD06"), ("p", "POOL1")]),
            done: "QMC2414",
            probe: vdisks,
        },
        Case {
            what: "delete switch ETHSW1",
            path: format!("/qsmc/confirm/switch-delete?ms={VS}&n=ETHSW1"),
            fields: ms(&[("n", "ETHSW1"), ("force", "on")]),
            done: "QMC2427",
            probe: switches,
        },
        Case {
            what: "delete image blank_4700MB.iso",
            path: format!("/qsmc/confirm/image-delete?ms={VS}&n=blank_4700MB.iso"),
            fields: ms(&[("n", "blank_4700MB.iso")]),
            done: "QMC2408",
            probe: images,
        },
        Case {
            what: "delete tape volume VTAP08",
            path: format!("/qsmc/confirm/volume-delete?ms={VS}&n=VTAP08"),
            fields: ms(&[("n", "VTAP08")]),
            done: "QMC2432",
            probe: volumes,
        },
        Case {
            what: "delete processor pool TESTPOOL",
            path: format!("/qsmc/confirm/proc-pool-delete?ms={VS}&n=TESTPOOL"),
            fields: ms(&[("n", "TESTPOOL")]),
            done: "QMC2441",
            probe: procpools,
        },
    ]
}

/// One case: by GET, without the token, with Cancel, then with OK.
fn one(lab: &mut Lab<'_>, c: &Case) -> Result<String, String> {
    let (fleet, b) = (&lab.fleet, &mut lab.c.browser);
    // Nothing on its way: no partition starting or shutting down.
    until("the partitions at rest", Duration::from_secs(60), || {
        Ok(host(fleet).parts.iter().all(|p| {
            matches!(
                p.status.state,
                PartitionState::Running | PartitionState::NotActivated
            )
        }))
    })?;
    let before = (c.probe)(fleet);
    let t = page(b, &c.path)?;
    if !t.contains("Press OK") || (c.probe)(fleet) != before {
        return Err(format!("{}: its confirmation's GET did something", c.what));
    }
    let mut bare: Vec<(&str, &str)> = c.fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    bare.push(("ok", "OK"));
    let r = b.post(&c.path, &bare)?;
    if r.status != 403 || !r.text().contains("QMC9011") || (c.probe)(fleet) != before {
        return Err(format!(
            "{}: a POST without the token answered {} ({})",
            c.what,
            r.status,
            message_of(&r.text())
        ));
    }
    let mut cancel: Vec<(&str, &str)> = c.fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    cancel.push(("cancel", "Cancel"));
    let r = submit(b, &c.path, &cancel)?;
    if r.status != 303 || (c.probe)(fleet) != before {
        return Err(format!(
            "{}: Cancel answered {} or did something",
            c.what, r.status
        ));
    }
    let r = submit(b, &c.path, &bare)?;
    expect(b, &r, c.done)?;
    until(
        &format!("{} at the host", c.what),
        Duration::from_secs(20),
        || Ok((c.probe)(fleet) != before),
    )?;
    Ok(format!("{} ({})", c.what, c.done))
}

/// Suite 5.
pub fn confirm(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-confirm")?;
    {
        let b = &mut lab.c.browser;
        let r = post_form(
            b,
            &format!("/qsmc/server/{VS}/virtual/tape/volume"),
            ("create", "Create volume"),
            &[("vn", "VTAP08"), ("vs", "10")],
        )?;
        expect(b, &r, "QMC2430")?;
    }
    let mut done = Vec::new();
    for c in cases() {
        done.push(one(&mut lab, &c)?);
    }
    // What the deletes kept and took.
    let vd = vdisks(&lab.fleet);
    if line_of(&vd, "POOL1/VD04").is_none() || line_of(&vd, "POOL1/VD03").is_some() {
        return Err(format!("the partitions' disks after their deletes: {vd}"));
    }
    lab.c.guest.kill();
    Ok(format!(
        "{} destructive actions each did nothing by its confirmation's GET, nothing by a POST \
         without the token (403, QMC9011) or with Cancel, and were done by OK: {}; QSTEST01's VD04 \
         kept, QSDEV01's VD03 deleted with it",
        done.len(),
        done.join(", ")
    ))
}
