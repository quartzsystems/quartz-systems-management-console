// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Suite 1: every page of this phase served, each link and form action
//! answering, no script but the navigation script and the terminal
//! window's, every form posting without one. Suite 9: the routes for each role. Suite 10: the list
//! never waits. Suite 11: a host before handoff 24.

use std::collections::{BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use webusers::{Matrix, Role, Task};

use super::{Lab, STANDALONE, VS, host, kept, lab, line_of, post_form, post_on, user};
use crate::suite::mgmt::{page, until};
use crate::suite::web::{message_of, shows};
use crate::web::Browser;
use crate::web::form::{OFF, forms};

/// The pages a crawl starts from: every page of this phase for
/// `QSHQVS1`, and `QSHQ400`'s where they apply.
fn seeds() -> Vec<String> {
    let mut v: Vec<String> = [
        "/qsmc/partitions",
        "/qsmc/groups",
        "/qsmc/groups/create",
        "/qsmc/group/LAB",
        "/qsmc/schedule",
        "/qsmc/schedule/add",
        "/qsmc/schedule?run=1",
        "/qsmc/confirm/schedule-remove?id=1",
        "/qsmc/confirm/group-delete?g=LAB",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    v.push(format!("/qsmc/partitions/create/step/1?ms={VS}"));
    v.extend((2..=9).map(|n| format!("/qsmc/partitions/create/step/{n}")));
    for p in [
        "ethernet",
        "ethernet/switch",
        "ethernet/switch?n=ETHSW0",
        "storage",
        "storage/create",
        "storage/pool",
        "storage/pool?n=POOL1",
        "storage/resize?p=POOL1&n=VD06",
        "optical",
        "optical/load?n=QS400_CUM_26290.iso",
        "optical/load?n=HLINV_SAVF_20261028.iso&unload=1",
        "tape",
        "tape/volume",
        "tape/volume?p=QSDEV01&s=6",
        "pools",
        "pools/change",
        "pools/change?id=1",
    ] {
        v.push(format!("/qsmc/server/{VS}/virtual/{p}"));
    }
    for id in 1..=3 {
        for p in [
            "general",
            "general?tab=other",
            "processors",
            "memory",
            "io",
            "vadapters",
            "settings",
            "activate",
            "activate?confirm=1",
            "shutdown",
            "shutdown?restart=1",
            "shutdown?confirm=1&how=delayed&delay=300",
            "profiles",
            "profiles?save=1",
            "edit?new=ethernet",
            "edit?new=scsi",
            "edit?io=1",
        ] {
            v.push(format!("/qsmc/partition/{VS}/{id}/{p}"));
        }
    }
    for c in [
        "partition-delete?ms=QSHQVS1&p=2",
        "adapter-remove?ms=QSHQVS1&p=1&s=2",
        "profile-delete?ms=QSHQVS1&p=1&n=big-test",
        "switch-delete?ms=QSHQVS1&n=ETHSW1",
        "vdisk-delete?ms=QSHQVS1&n=VD06&p=POOL1",
        "vdisk-unassign?ms=QSHQVS1&n=VD03&p=POOL1",
        "image-delete?ms=QSHQVS1&n=blank_4700MB.iso",
        "tape-remove?ms=QSHQVS1&p=QSDEV01&s=6",
        "volume-delete?ms=QSHQVS1&n=VTAP02",
        "proc-pool-delete?ms=QSHQVS1&n=TESTPOOL",
    ] {
        v.push(format!("/qsmc/confirm/{c}"));
    }
    v.push(format!("/qsmc/partition/{STANDALONE}/0/general"));
    v.push(format!("/qsmc/server/{STANDALONE}/virtual/ethernet"));
    v
}

/// Whether `path` is a page of this phase, whose links are followed.
fn ours(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or("");
    p.starts_with("/qsmc/partition")
        || p.starts_with("/qsmc/group")
        || p.starts_with("/qsmc/schedule")
        || p.contains("/virtual")
        || webpages::partitions::confirm::ROUTES
            .iter()
            .any(|r| p.ends_with(r))
        || webpages::vres::confirm::ROUTES
            .iter()
            .any(|r| p.ends_with(r))
        || p.ends_with("/confirm/schedule-remove")
        || p.ends_with("/confirm/group-delete")
}

/// Whether `path` is a terminal window, the one page with a script.
pub(crate) fn window(path: &str) -> bool {
    let p = path.split('?').next().unwrap_or("");
    p.starts_with("/qsmc/terminal/") && p.matches('/').count() == 4
}

/// A crawl's key: a page's other sorts, pages and rows are the same page.
pub(crate) fn key(path: &str) -> String {
    let (p, q) = path.split_once('?').unwrap_or((path, ""));
    let kept: Vec<&str> = q
        .split('&')
        .filter(|kv| {
            !kv.is_empty()
                && !["sort=", "page=", "rows=", "s=", "n=", "ms="]
                    .iter()
                    .any(|k| kv.starts_with(k) && p.ends_with("/partitions"))
                && !kv.starts_with("sort=")
                && !kv.starts_with("page=")
                && !kv.starts_with("rows=")
        })
        .collect();
    format!("{p}?{}", kept.join("&"))
}

/// The links of a page: `<a href>`, `<img src>`, `formaction`. What is
/// not a page (a file downloaded, a picture) has none.
pub(crate) fn links(html: &str) -> Vec<String> {
    // Bytes that were not text, or markup the tokens cannot end.
    if html.contains('\u{fffd}') {
        return Vec::new();
    }
    std::panic::catch_unwind(|| qsweb::golden::tokens(html))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|t| match t {
            qsweb::golden::Tok::Open(n, a) if n == "a" || n == "img" || n == "input" => a
                .get("href")
                .or_else(|| a.get("src"))
                .or_else(|| a.get("formaction"))
                .cloned(),
            _ => None,
        })
        .map(|h| h.split('#').next().unwrap_or("").replace("&amp;", "&"))
        .filter(|h| h.starts_with("/qsmc/"))
        .filter_map(crate::web::crawled)
        .collect()
}

/// What the crawl saw.
#[derive(Default)]
pub(crate) struct Crawl {
    pub(crate) pages: usize,
    pub(crate) other: usize,
    pub(crate) forms: usize,
    pub(crate) posted: usize,
    pub(crate) windows: usize,
}

/// Check `html`'s forms, served at `here`: each posts with the token, has
/// a button, and an action that answers; one with Cancel is posted.
pub(crate) fn check_forms(
    b: &mut Browser,
    html: &str,
    here: &str,
    c: &mut Crawl,
) -> Result<(), String> {
    for f in forms(html, here) {
        c.forms += 1;
        // The frame's own search is a GET, which needs no token.
        if f.method == "get" {
            continue;
        }
        if f.method != "post" {
            return Err(format!("{here}: a form posts by {}", f.method));
        }
        if !f.has_csrf() {
            return Err(format!("{here}: a form without its token"));
        }
        if f.submits.is_empty() {
            return Err(format!("{here}: a form with no button"));
        }
        let mut actions = vec![f.action.clone()];
        actions.extend(
            f.submits
                .iter()
                .filter(|s| !s.2.is_empty())
                .map(|s| s.2.clone()),
        );
        for a in actions {
            let to = a.replace("&amp;", "&");
            let r = b.get(&to)?;
            if r.status == 404 || r.status >= 500 {
                return Err(format!(
                    "{here}: a form's action {to} answered {}",
                    r.status
                ));
            }
        }
        if f.submits_as("cancel") && f.enctype.is_empty() {
            let r = post_on(b, html, here, ("cancel", "Cancel"), &[])?;
            if !(r.status == 303 || r.status == 200) {
                return Err(format!("{here}: its Cancel answered {}", r.status));
            }
            c.posted += 1;
        }
    }
    Ok(())
}

/// Suite 1.
#[allow(clippy::too_many_lines, reason = "one crawl")]
pub fn pages(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-pages")?;
    let b = &mut lab.c.browser;
    // A group and an operation, so their pages and rows are there.
    let r = post_form(
        b,
        "/qsmc/groups/create",
        ("ok", "OK"),
        &[("g", "LAB"), ("m1", "on")],
    )?;
    super::expect(b, &r, "QMC3301")?;
    let add = page(b, "/qsmc/schedule/add")?;
    let r = post_on(
        b,
        &add,
        "/qsmc/schedule/add",
        ("ok", "OK"),
        &[
            ("r", "once"),
            ("op", "Activate partition"),
            ("tg", "QSTEST01 on QSHQVS1"),
            ("p", "default"),
        ],
    )?;
    super::expect(b, &r, "QMC3401")?;
    let mut seen = BTreeSet::new();
    let mut queue: VecDeque<String> = seeds().into();
    let mut c = Crawl::default();
    while let Some(path) = queue.pop_front() {
        let bare = path.split('?').next().unwrap_or("").to_string();
        if bare.starts_with("/qsmc/signoff") || bare.ends_with("/ws") || !seen.insert(key(&path)) {
            continue;
        }
        let a = b.get(&path)?;
        if crate::web::opened(&path, a.status)? {
            continue;
        }
        if !ours(&path) {
            if a.status == 404 || a.status >= 500 {
                return Err(format!("{path} answered {}", a.status));
            }
            if crate::web::scripted(&a.text()) {
                if !window(&path) {
                    return Err(format!("{path} carries a script"));
                }
                c.windows += 1;
            }
            c.other += 1;
            continue;
        }
        // A standalone system's partition is the system's own page.
        let a = if a.status == 303 && path.contains(STANDALONE) {
            crate::suite::web::follow(b, &a)?
        } else {
            a
        };
        // Navigator leaves for the partition's own address.
        if a.status == 303 && bare.ends_with("/navigator") {
            continue;
        }
        if a.status != 200 {
            return Err(format!(
                "{path} answered {}: {}",
                a.status,
                message_of(&a.text())
            ));
        }
        c.pages += 1;
        let text = a.text();
        if crate::web::scripted(&text) {
            if !window(&path) {
                return Err(format!("{path} carries a script"));
            }
            c.windows += 1;
        }
        if window(&path) {
            continue;
        }
        check_forms(b, &text, &path, &mut c)?;
        for l in links(&text) {
            queue.push_back(l);
        }
        if c.pages + c.other > 3000 {
            return Err("more than 3000 pages: the crawl does not end".into());
        }
    }
    lab.c.guest.kill();
    Ok(format!(
        "{} pages of this phase (QSHQVS1's partitions, profiles, virtual resources, the wizard, \
         groups, schedules, confirmations; QSHQ400's where they apply) each answered 200, and {} \
         pages they link to answered; {} forms each post with the token, have a button and an \
         action that answers, {} of them posted with Cancel; no page carries a script of its own ({} terminal \
         windows reached, which do)",
        c.pages, c.other, c.forms, c.posted, c.windows
    ))
}

/// One route of suite 9: what a GET needs, and a POST's fields and the
/// task it needs. A POST's fields change nothing: invalid entries, or no
/// OK.
struct R {
    path: &'static str,
    get: Task,
    post: Option<(Task, &'static [(&'static str, &'static str)])>,
}

const fn r(
    path: &'static str,
    get: Task,
    post: Option<(Task, &'static [(&'static str, &'static str)])>,
) -> R {
    R { path, get, post }
}

use Task::{Activate as A, CreatePartitions as C, View as V, VirtualResources as VR};

/// The routes of this phase and the tasks `docs/WEBUSERS.md` gives them.
const ROUTES: &[R] = &[
    r(
        "/qsmc/partitions",
        V,
        Some((V, &[("act", "--Select action--")])),
    ),
    r("/qsmc/groups", V, None),
    r(
        "/qsmc/group/LAB",
        V,
        Some((V, &[("act", "--Select action--")])),
    ),
    r("/qsmc/groups/create", A, Some((A, &[]))),
    r(
        "/qsmc/confirm/group-delete?g=LAB",
        A,
        Some((A, &[("g", "LAB")])),
    ),
    r("/qsmc/schedule", V, None),
    r("/qsmc/schedule/add", V, None),
    r("/qsmc/schedule?run=1", A, Some((A, &[("run", "1")]))),
    r(
        "/qsmc/confirm/schedule-remove?id=1",
        A,
        Some((A, &[("id", "1")])),
    ),
    r(
        "/qsmc/partitions/create/step/1?ms=QSHQVS1",
        C,
        Some((C, &[("id", "x"), ("next", "Next")])),
    ),
    r("/qsmc/partitions/create/step/3", C, None),
    r(
        "/qsmc/partition/QSHQVS1/2/general",
        V,
        Some((C, &[("n", ""), ("apply", "Apply")])),
    ),
    r(
        "/qsmc/partition/QSHQVS1/2/processors",
        V,
        Some((C, &[("vmin", "x"), ("apply", "Apply")])),
    ),
    r(
        "/qsmc/partition/QSHQVS1/2/memory",
        V,
        Some((C, &[("min", "x"), ("apply", "Apply")])),
    ),
    r(
        "/qsmc/partition/QSHQVS1/2/io",
        V,
        Some((C, &[("apply", "Apply")])),
    ),
    r("/qsmc/partition/QSHQVS1/2/vadapters", V, None),
    r(
        "/qsmc/partition/QSHQVS1/2/settings",
        V,
        Some((C, &[("bm", "Open firmware"), ("apply", "Apply")])),
    ),
    r(
        "/qsmc/partition/QSHQVS1/2/edit?new=ethernet",
        V,
        Some((C, &[("new", "ethernet"), ("vlan", "x"), ("ok", "OK")])),
    ),
    r(
        "/qsmc/partition/QSHQVS1/2/profiles",
        V,
        Some((
            C,
            &[("do", "copy"), ("from", "install"), ("n", ""), ("ok", "OK")],
        )),
    ),
    r(
        "/qsmc/partition/QSHQVS1/2/activate",
        V,
        Some((A, &[("prof", "install"), ("ok", "OK")])),
    ),
    r(
        "/qsmc/partition/QSHQVS1/1/shutdown",
        V,
        Some((A, &[("how", "delayed"), ("delay", "300"), ("ok", "OK")])),
    ),
    r(
        "/qsmc/confirm/partition-delete?ms=QSHQVS1&p=2",
        C,
        Some((C, &[("ms", "QSHQVS1"), ("p", "2")])),
    ),
    r(
        "/qsmc/confirm/adapter-remove?ms=QSHQVS1&p=1&s=2",
        C,
        Some((C, &[("ms", "QSHQVS1"), ("p", "1"), ("s", "2")])),
    ),
    r(
        "/qsmc/confirm/profile-delete?ms=QSHQVS1&p=1&n=big-test",
        C,
        Some((C, &[("ms", "QSHQVS1"), ("p", "1"), ("n", "big-test")])),
    ),
    r("/qsmc/server/QSHQVS1/virtual/ethernet", V, None),
    r(
        "/qsmc/server/QSHQVS1/virtual/ethernet/switch",
        V,
        Some((VR, &[("to", "")])),
    ),
    r("/qsmc/server/QSHQVS1/virtual/storage", V, None),
    r(
        "/qsmc/server/QSHQVS1/virtual/storage/pool",
        V,
        Some((VR, &[("ok", "Create")])),
    ),
    r(
        "/qsmc/server/QSHQVS1/virtual/storage/create",
        V,
        Some((VR, &[("n", ""), ("ok", "OK")])),
    ),
    r(
        "/qsmc/server/QSHQVS1/virtual/storage/resize?p=POOL1&n=VD06",
        V,
        Some((
            VR,
            &[("p", "POOL1"), ("n", "VD06"), ("s", "1"), ("ok", "Resize")],
        )),
    ),
    r("/qsmc/server/QSHQVS1/virtual/optical", V, None),
    r(
        "/qsmc/server/QSHQVS1/virtual/optical/load?n=QS400_CUM_26290.iso",
        V,
        Some((
            VR,
            &[("n", "QS400_CUM_26290.iso"), ("d", "none"), ("ok", "OK")],
        )),
    ),
    r("/qsmc/server/QSHQVS1/virtual/tape", V, None),
    r(
        "/qsmc/server/QSHQVS1/virtual/tape/volume",
        V,
        Some((VR, &[("vs", ""), ("create", "Create")])),
    ),
    r("/qsmc/server/QSHQVS1/virtual/pools", V, None),
    r(
        "/qsmc/server/QSHQVS1/virtual/pools/change",
        V,
        Some((VR, &[("to", ""), ("c", "x"), ("ok", "OK")])),
    ),
    r(
        "/qsmc/confirm/switch-delete?ms=QSHQVS1&n=ETHSW1",
        VR,
        Some((VR, &[("ms", "QSHQVS1"), ("n", "ETHSW1")])),
    ),
    r(
        "/qsmc/confirm/vdisk-delete?ms=QSHQVS1&n=VD06&p=POOL1",
        VR,
        Some((VR, &[("ms", "QSHQVS1"), ("n", "VD06"), ("p", "POOL1")])),
    ),
    r(
        "/qsmc/confirm/vdisk-unassign?ms=QSHQVS1&n=VD03&p=POOL1",
        VR,
        Some((VR, &[("ms", "QSHQVS1"), ("n", "VD03"), ("p", "POOL1")])),
    ),
    r(
        "/qsmc/confirm/image-delete?ms=QSHQVS1&n=blank_4700MB.iso",
        VR,
        Some((VR, &[("ms", "QSHQVS1"), ("n", "blank_4700MB.iso")])),
    ),
    r(
        "/qsmc/confirm/tape-remove?ms=QSHQVS1&p=QSDEV01&s=6",
        VR,
        Some((VR, &[("ms", "QSHQVS1"), ("p", "QSDEV01"), ("s", "6")])),
    ),
    r(
        "/qsmc/confirm/volume-delete?ms=QSHQVS1&n=VTAP02",
        VR,
        Some((VR, &[("ms", "QSHQVS1"), ("n", "VTAP02")])),
    ),
    r(
        "/qsmc/confirm/proc-pool-delete?ms=QSHQVS1&n=TESTPOOL",
        VR,
        Some((VR, &[("ms", "QSHQVS1"), ("n", "TESTPOOL")])),
    ),
];

/// Suite 9.
#[allow(clippy::too_many_lines, reason = "one table, every role")]
pub fn roles(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-roles")?;
    let b = &mut lab.c.browser;
    let r = post_form(
        b,
        "/qsmc/groups/create",
        ("ok", "OK"),
        &[("g", "LAB"), ("m1", "on")],
    )?;
    super::expect(b, &r, "QMC3301")?;
    let r = post_form(
        b,
        "/qsmc/schedule/add",
        ("ok", "OK"),
        &[
            ("r", "once"),
            ("op", "Activate partition"),
            ("tg", "QSTEST01 on QSHQVS1"),
            ("p", "default"),
        ],
    )?;
    super::expect(b, &r, "QMC3401")?;
    let matrix = Matrix::default();
    let mut who: Vec<(Role, Browser)> = vec![(Role::Administrator, b.clone_session())];
    for role in [Role::Operator, Role::Viewer, Role::Service] {
        let id = role.label().to_lowercase();
        who.push((role, user(b, &id, role.label(), "Parts-roles-42")?));
    }
    let (mut pairs, mut refused) = (0, 0);
    let mut wrong = Vec::new();
    for (role, nb) in &mut who {
        for route in ROUTES {
            let a = nb.get(route.path)?;
            let want = matrix.allows(*role, route.get);
            let ok = if want {
                a.status == 200 || a.status == 303
            } else {
                a.status == 403 && shows(&a.text(), "QMC9010")
            };
            if !ok {
                wrong.push(format!("{} GET {}: {}", role.label(), route.path, a.status));
            }
            pairs += 1;
            refused += usize::from(!want);
            let Some((task, fields)) = route.post else {
                continue;
            };
            let token = nb.csrf("/qsmc/partitions")?;
            let mut all = vec![("csrf", token.as_str())];
            all.extend_from_slice(fields);
            let a = nb.post(route.path, &all)?;
            let want = matrix.allows(*role, task);
            let ok = if want {
                a.status != 403 && a.status != 404 && a.status < 500
            } else {
                a.status == 403 && shows(&a.text(), "QMC9010")
            };
            if !ok {
                wrong.push(format!(
                    "{} POST {}: {} ({})",
                    role.label(),
                    route.path,
                    a.status,
                    message_of(&a.text())
                ));
            }
            pairs += 1;
            refused += usize::from(!want);
        }
        // The upload, a multipart POST to Virtual Optical.
        let path = "/qsmc/server/QSHQVS1/virtual/optical";
        let token = nb.csrf(path)?;
        let a = nb.stream_upload(
            path,
            &crate::web::stream::Upload {
                csrf: token,
                name: "ROLES.iso".into(),
                bytes: 64 * 2048,
                announce: None,
                cut: None,
                head_only: *role != Role::Administrator,
                label: "ROLES".into(),
            },
        )?;
        let status = a.map_or(0, |a| a.status);
        let want = matrix.allows(*role, Task::VirtualResources);
        if (status == 403) == want {
            wrong.push(format!("{} upload: {status}", role.label()));
        }
        pairs += 1;
    }
    lab.c.guest.kill();
    if !wrong.is_empty() {
        return Err(format!(
            "{} of {pairs} route and role pairs not as the matrix says: {}",
            wrong.len(),
            wrong.join("; ")
        ));
    }
    Ok(format!(
        "{pairs} route, method and role pairs of this phase for Administrator, Operator, Viewer \
         and Service, each as the matrix says: {refused} Not authorized (403, QMC9010), the rest \
         served"
    ))
}

/// Suite 10.
pub fn lists(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-lists")?;
    lab.fleet.tell(0, "stall on");
    let b = &mut lab.c.browser;
    let t0 = Instant::now();
    let mut slowest = Duration::ZERO;
    let mut away = 0;
    let mut text = String::new();
    while t0.elapsed() < Duration::from_secs(150) {
        for path in [
            "/qsmc/partitions",
            "/qsmc/groups",
            "/qsmc/server/QSHQVS1/virtual/storage",
        ] {
            let t = Instant::now();
            let p = page(b, path)?;
            slowest = slowest.max(t.elapsed());
            if path == "/qsmc/partitions" {
                text = p;
            }
        }
        away = ["QSDEV01", "QSTEST01", "QSPROD01"]
            .iter()
            .filter(|n| row(&text, n).contains("No connection"))
            .count();
        if away == 3 {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    lab.fleet.tell(0, "stall off");
    if slowest > Duration::from_secs(2) {
        return Err(format!("a list took {} ms", slowest.as_millis()));
    }
    if away < 3 || row(&text, STANDALONE).contains("No connection") {
        return Err(format!(
            "{away} of QSHQVS1's 3 partitions marked No connection; QSHQ400's row: {}",
            row(&text, STANDALONE)
        ));
    }
    lab.c.guest.kill();
    Ok(format!(
        "with QSHQVS1 stalled, Partitions, Custom groups and Virtual Storage each answered within \
         {} ms throughout; QSHQVS1's three partitions kept their rows marked No connection after \
         {} s, QSHQ400's row not",
        slowest.as_millis(),
        t0.elapsed().as_secs()
    ))
}

/// The list's row whose name is `name`.
fn row<'t>(page: &'t str, name: &str) -> &'t str {
    let Some(at) = page.find(&format!(">{name}</a>")) else {
        return "";
    };
    let start = page[..at].rfind("<tr").unwrap_or(at);
    let end = page[at..].find("</tr>").map_or(page.len(), |e| at + e);
    &page[start..end]
}

/// Whether `t` says QMC1114 and the function in its words.
fn lacks(t: &str, words: &str) -> bool {
    shows(t, "QMC1114") && super::plain(t).contains(words)
}

/// Suite 11.
#[allow(clippy::too_many_lines, reason = "one function a step")]
pub fn older(run: &crate::suite::base::Run<'_>) -> Result<String, String> {
    let mut lab = lab(run, "parts-older")?;
    let sessions = |lab: &Lab<'_>| {
        host(&lab.fleet)
            .lines
            .iter()
            .filter(|l| l.contains("session from") && l.contains("opened"))
            .count()
    };
    let before = sessions(&lab);
    lab.fleet.tell(0, "withhold new");
    until(
        "the session again without the new bits",
        Duration::from_secs(90),
        || {
            Ok(sessions(&lab) > before
                && page(&mut lab.c.browser, "/qsmc/connectivity")?.contains(">Connected<"))
        },
    )?;
    std::thread::sleep(Duration::from_secs(3));
    let (fleet, b) = (&lab.fleet, &mut lab.c.browser);
    // Every page of this phase is still served.
    let mut served = 0;
    for p in seeds() {
        if p.contains(STANDALONE) || p.contains("LAB") || p.contains("id=1") || p.contains("run=1")
        {
            continue;
        }
        let a = b.get(&p)?;
        if a.status != 200 && a.status != 303 {
            return Err(format!("{p} answered {} for the older host", a.status));
        }
        served += 1;
    }
    let mut missing = Vec::new();
    let mut found = Vec::new();
    let mut check = |what: &str, t: &str, words: &str| {
        if lacks(t, words) {
            found.push(what.to_string());
        } else {
            missing.push(format!("{what}: {}", message_of(t)));
        }
    };
    // Power-on settings.
    let r = post_form(
        b,
        &format!("/qsmc/server/{VS}/power"),
        ("apply", "Apply"),
        &[("auto", "none")],
    )?;
    let t = crate::suite::web::follow(b, &r)?.text();
    check(
        "power-on settings",
        &t,
        "power-on settings set from a console",
    );
    if !host(fleet).power_on.start_partitions {
        return Err("the power-on settings reached the older host".into());
    }
    // Switches and storage pools.
    let r = post_form(
        b,
        &format!("/qsmc/server/{VS}/virtual/ethernet/switch"),
        ("ok", "OK"),
        &[("to", "OLDSW"), ("v", "70")],
    )?;
    let t = crate::suite::web::follow(b, &r)?.text();
    check(
        "a switch made",
        &t,
        "virtual switches made and changed from a console",
    );
    let r = post_form(
        b,
        &format!("/qsmc/server/{VS}/virtual/storage/pool"),
        ("ok", "Create"),
        &[("to", "OLDPOOL"), ("pr", "None"), ("d_sdi", "on")],
    )?;
    let t = crate::suite::web::follow(b, &r)?.text();
    check(
        "a storage pool made",
        &t,
        "storage pools made and changed from a console",
    );
    if line_of(&kept(fleet, "switches"), "OLDSW").is_some()
        || line_of(&kept(fleet, "pools"), "OLDPOOL").is_some()
    {
        return Err("a switch or pool reached the older host".into());
    }
    // The Settings tab.
    let tab = format!("/qsmc/partition/{VS}/1/settings");
    let r = post_form(b, &tab, ("apply", "Apply"), &[("cp", OFF)])?;
    let t = crate::suite::web::follow(b, &r)?.text();
    check("the Settings tab", &t, "a partition's settings");
    // Create with an ID: the wizard to its end.
    let mut step = page(b, &format!("/qsmc/partitions/create/step/1?ms={VS}"))?;
    let mut at = format!("/qsmc/partitions/create/step/1?ms={VS}");
    for n in 1..=8 {
        let set: &[(&str, &str)] = if n == 1 {
            &[("id", "9"), ("n", "QSOLD09")]
        } else {
            &[]
        };
        let r = post_on(b, &step, &at, ("next", "Next >"), set)?;
        at = format!("/qsmc/partitions/create/step/{}", n + 1);
        if r.status != 303 {
            return Err(format!("the wizard's step {n}: {}", message_of(&r.text())));
        }
        step = page(b, &at)?;
    }
    let r = post_on(b, &step, &at, ("finish", "Finish"), &[])?;
    let t = crate::suite::web::follow(b, &r)?.text();
    check("a partition made at its ID", &t, "a partition's settings");
    if host(fleet).part_ref(9).is_some() {
        return Err("the partition was made on the older host".into());
    }
    // Activation times: General's Running since, Manage profiles' Last
    // activated.
    let t = page(b, &format!("/qsmc/partition/{VS}/1/general"))?;
    let p = page(b, &format!("/qsmc/partition/{VS}/1/profiles"))?;
    check("activation times", &format!("{t}{p}"), "activation times");
    // The control panel's list of functions.
    let t = page(b, &format!("/qsmc/partition/{VS}/1/panel"))?;
    check(
        "the panel's list",
        &t,
        "the list of its control panel functions",
    );
    lab.c.guest.kill();
    if !missing.is_empty() {
        return Err(format!(
            "{served} pages served for the older host; QMC1114 named {}; not for: {}",
            found.join(", "),
            missing.join("; ")
        ));
    }
    Ok(format!(
        "with QSHQVS1's hello lacking bits 19 to 24, {served} pages of this phase were served, and \
         QMC1114 named what it lacks for {}; nothing of them reached the host",
        found.join(", ")
    ))
}
