// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The breadcrumb as links: each entry but the last leads to its page,
//! worked out from the entries before it. `qsweb` draws the crumb as
//! text; the served page has its links put in (the prototype's crumbs
//! are text, so the golden files keep them so).

use qsweb::html::esc;
use webd::product::{NAV, PRODUCT};

/// The route of each entry of `crumb` (after the root), `None` for the
/// last and for one whose page is not known. `route` is the request's,
/// for what only it names (a partition's host and ID).
pub fn links(crumb: &[String], route: &str) -> Vec<Option<String>> {
    let enc = |s: &str| qsweb::http::form::encode(s);
    let mut out = Vec::with_capacity(crumb.len());
    for (i, label) in crumb.iter().enumerate() {
        if i + 1 == crumb.len() {
            out.push(None);
            break;
        }
        let back = |n: usize| i.checked_sub(n).map(|j| crumb[j].as_str());
        let l = label.as_str();
        let to = if let Some(g) = NAV.iter().find(|g| g.label == l) {
            g.items.first().map(|(_, r)| (*r).to_string())
        } else if let Some((_, r)) = NAV.iter().flat_map(|g| g.items).find(|(n, _)| *n == l) {
            Some((*r).to_string())
        } else if l == "Scheduled operations" {
            Some("/schedule".into())
        } else if back(1) == Some("Servers") {
            Some(format!("/server/{}/general", enc(l)))
        } else if back(2) == Some("Servers") && l == crate::systems::props::VIRTUAL.1 {
            Some(format!("/server/{}/virtual/ethernet", enc(&crumb[i - 1])))
        } else if back(2) == Some("Servers")
            && let Some((r, _)) = crate::systems::props::TABS
                .iter()
                .chain(crate::systems::props::HOST_TABS.iter())
                .chain(crate::systems::props::STANDALONE_TABS.iter())
                .find(|(_, n)| *n == l)
        {
            Some(format!("/server/{}/{r}", enc(&crumb[i - 1])))
        } else if back(3) == Some("Servers")
            && back(1) == Some(crate::systems::props::VIRTUAL.1)
            && let Some((r, _)) = crate::vres::TABS.iter().find(|(_, n)| *n == l)
        {
            Some(format!("/server/{}/virtual/{r}", enc(&crumb[i - 2])))
        } else if back(1) == Some("Partitions") {
            partition(route)
        } else if back(2) == Some("Partitions") && l == "Terminal window" {
            window(route)
        } else if back(1) == Some(crate::plans::TITLE) {
            Some(crate::plans::route_of(l, ""))
        } else if back(1) == Some("Custom groups") {
            Some(format!("/group/{}", enc(l)))
        } else if back(1) == Some("Serviceable events") {
            Some(format!("/event/{}", enc(l)))
        } else {
            None
        };
        out.push(to);
    }
    out
}

/// `/partition/<host>/<id>/general` of a partition's page's route, or
/// of its terminal window's.
fn partition(route: &str) -> Option<String> {
    let mut p = route
        .strip_prefix("/partition/")
        .or_else(|| route.strip_prefix("/terminal/"))?
        .split('/');
    let (host, id) = (p.next()?, p.next()?);
    id.parse::<u32>().ok()?;
    Some(format!("/partition/{host}/{id}/general"))
}

/// `/terminal/<host>/<id>`, the window, of a page under it.
fn window(route: &str) -> Option<String> {
    let mut p = route.strip_prefix("/terminal/")?.split('/');
    let (host, id) = (p.next()?, p.next()?);
    id.parse::<u32>().ok()?;
    Some(format!("/terminal/{host}/{id}"))
}

/// The crumb's line: the root to Welcome, then each entry, linked when
/// [`links`] knows its page.
pub fn line(crumb: &[String], route: &str) -> String {
    let mut parts = vec![format!(
        "<a href=\"{}\">{}</a>",
        esc(&PRODUCT.url("/welcome")),
        esc(PRODUCT.crumb_root)
    )];
    for (label, to) in crumb.iter().zip(links(crumb, route)) {
        parts.push(match to {
            Some(r) => format!("<a href=\"{}\">{}</a>", esc(&PRODUCT.url(&r)), esc(label)),
            None => esc(label),
        });
    }
    parts.join(" &gt; ")
}

/// `html`, a page in the frame, with its crumb's line as links.
pub fn linked(html: &str, crumb: &[String], route: &str) -> String {
    const OPEN: &str = "<div class=\"crumb\">";
    let Some(start) = html.find(OPEN).map(|s| s + OPEN.len()) else {
        return html.to_string();
    };
    let Some(end) = html[start..].find("</div>").map(|e| start + e) else {
        return html.to_string();
    };
    format!("{}{}{}", &html[..start], line(crumb, route), &html[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_terminal_window_s_crumb_leads_to_its_partition() {
        let c = s(&[
            "Systems Management",
            "Partitions",
            "QS400A",
            "Terminal window",
        ]);
        assert_eq!(
            links(&c, "/terminal/QSVS1/1"),
            [
                Some("/servers".to_string()),
                Some("/partitions".into()),
                Some("/partition/QSVS1/1/general".into()),
                None
            ]
        );
        let mut copy = c.clone();
        copy.push("Copy".into());
        assert_eq!(
            links(&copy, "/terminal/QSVS1/1/copy")[3].as_deref(),
            Some("/terminal/QSVS1/1")
        );
    }

    #[test]
    fn each_entry_but_the_last_leads_to_its_page() {
        let c = s(&[
            "Systems Management",
            "Servers",
            "QSVS1",
            "Virtual resources",
            "Virtual Storage",
        ]);
        assert_eq!(
            links(&c, "/server/QSVS1/virtual/storage"),
            [
                Some("/servers".into()),
                Some("/servers".into()),
                Some("/server/QSVS1/general".into()),
                Some("/server/QSVS1/virtual/ethernet".into()),
                None
            ]
        );
        let c = s(&["Systems Management", "Servers", "QSVS1", "Memory"]);
        assert_eq!(links(&c, "")[2], Some("/server/QSVS1/general".into()));
        let c = s(&["Systems Management", "Partitions", "QSDEV01", "Memory"]);
        assert_eq!(
            links(&c, "/partition/QSVS1/3/memory")[2],
            Some("/partition/QSVS1/3/general".into())
        );
        let c = s(&["Service Management", "Serviceable events", "1001", "Repair"]);
        assert_eq!(links(&c, "")[2], Some("/event/1001".into()));
        let c = s(&["Systems Management", "System Plans", "a b", "Edit"]);
        assert_eq!(links(&c, "")[2], Some(crate::plans::route_of("a b", "")));
        assert_eq!(links(&s(&["Unknown", "Page"]), ""), [None, None]);
    }

    #[test]
    fn the_line_replaces_the_frame_s_text_escaped() {
        let html = "<p>x</p><div class=\"crumb\">QS/MC &gt; Servers &gt; A&lt;</div><h1>";
        let c = s(&["Systems Management", "Servers", "A<"]);
        assert_eq!(
            linked(html, &c, ""),
            "<p>x</p><div class=\"crumb\"><a href=\"/qsmc/welcome\">QS/MC</a> &gt; \
             <a href=\"/qsmc/servers\">Systems Management</a> &gt; \
             <a href=\"/qsmc/servers\">Servers</a> &gt; A&lt;</div><h1>"
        );
    }
}
