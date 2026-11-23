// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Baselines, recorded and not held to a threshold (Phase 8, Section 8):
//! the first page after the machine starts, sign-on, and the Partitions
//! list with 25, 500 and 5,000 partitions known. Each service's memory
//! and processor are the soak's (`sec-soak`); base ready is `status`'s.

use std::time::{Duration, Instant};

use crate::suite::base::Run;
use crate::suite::mgmt::{Fleet, keep, until};
use crate::suite::web::{ADMIN, PASSWORD, signed_on, start};
use crate::web::Browser;

/// The median of `n` timings of `f`, in milliseconds.
fn median(n: usize, mut f: impl FnMut() -> Result<(), String>) -> Result<u128, String> {
    let mut took = Vec::new();
    for _ in 0..n {
        let at = Instant::now();
        f()?;
        took.push(at.elapsed().as_millis());
    }
    took.sort_unstable();
    Ok(took[took.len() / 2])
}

/// The Partitions list's time once `known` partitions are on it.
fn list(b: &mut Browser, known: usize) -> Result<String, String> {
    let pages = known.div_ceil(10);
    until(
        &format!("{known} partitions on the list"),
        Duration::from_secs(600),
        || {
            Ok(b.get("/qsmc/partitions?rows=10")?
                .text()
                .contains(&format!("Page 1 of {pages}")))
        },
    )?;
    let first = median(5, || {
        let a = b.get("/qsmc/partitions")?;
        if a.status == 200 {
            Ok(())
        } else {
            Err(format!("the list answered {}", a.status))
        }
    })?;
    let last = median(5, || {
        b.get(&format!("/qsmc/partitions?page={}", known.div_ceil(25)))
            .map(|_| ())
    })?;
    Ok(format!(
        "{known} partitions: the list in {first} ms, its last page in {last} ms"
    ))
}

/// The suite.
pub fn baselines(run: &Run<'_>) -> Result<String, String> {
    // 25, then 500, then 5,000 partitions: host 1 has 25, hosts 2 to
    // 10 and 12 to 101 have 50, host 11 has 25.
    let parts = |n: u32| if n == 1 || n == 11 { 25 } else { 50 };
    let fleet = Fleet::start(
        (1..=101)
            .map(|n| sim::fixture::fleet(n, parts(n)))
            .collect(),
    )?;
    let t0 = Instant::now();
    let mut c = start(run, "sec-baselines")?;
    let b = &mut c.browser;
    until("the first page", Duration::from_secs(120), || {
        Ok(b.get("/qsmc/signon")?.status == 200)
    })?;
    let page = t0.elapsed().as_millis();
    signed_on(b, ADMIN, PASSWORD)?;
    let sign_on = median(3, || {
        let mut s = b.fresh();
        let a = s.sign_on(ADMIN, PASSWORD)?;
        if a.status == 303 {
            Ok(())
        } else {
            Err(format!("sign-on answered {}", a.status))
        }
    })?;
    let mut said = vec![
        format!("the sign-on page {page} ms after the machine was started"),
        format!("sign-on {sign_on} ms"),
    ];
    keep(b, &fleet, 0)?;
    said.push(list(b, 25)?);
    for i in 1..11 {
        keep(b, &fleet, i)?;
    }
    said.push(list(b, 500)?);
    for i in 11..101 {
        keep(b, &fleet, i)?;
    }
    said.push(list(b, 5000)?);
    c.guest.kill();
    Ok(said.join("; "))
}
