// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! Custom groups (`groups.html`, `group-create.html`, `group-view.html`):
//! the console's own named sets of managed systems and partitions, in
//! `groups.toml`. A group's page is the Partitions list of its members;
//! a partition deleted at its host leaves every group it was in.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use qsweb::frame::{Ctx, Page};
use qsweb::html::esc;
use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Cell, Col, List, list_table, paging, status};

use mgmtproto::Kept;
use mgmtproto::kept::Kind as SystemKind;
use webusers::Task;

use crate::app::routes::{Rq, lacks};

/// One member: a whole system, or one of its partitions by ID and name
/// (both, so a partition made later at the same ID is not taken for it).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// The managed system.
    pub system: String,
    /// The partition's ID; none for the whole system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition: Option<u32>,
    /// The partition's name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

/// A group.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// Its name.
    pub name: String,
    /// What it is for.
    #[serde(default)]
    pub description: String,
    /// Who made it.
    #[serde(default)]
    pub owner: String,
    /// Its members.
    #[serde(default)]
    pub members: Vec<Member>,
}

/// `groups.toml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Groups {
    /// Every group.
    #[serde(default, rename = "group")]
    pub groups: Vec<Group>,
}

impl state::Document for Groups {
    const FILE: &'static str = "groups.toml";
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [state::Migration] = &[];

    fn validate(&self) -> Result<(), String> {
        for g in &self.groups {
            if g.name.is_empty()
                || g.name.len() > 32
                || !g.name.bytes().all(|b| b.is_ascii_graphic())
            {
                return Err(format!(
                    "{}: a group's name is 1 to 32 characters, no spaces",
                    g.name
                ));
            }
        }
        Ok(())
    }
}

fn load(rq: &Rq<'_>) -> Groups {
    rq.app
        .store()
        .load::<Groups>()
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn save(rq: &Rq<'_>, g: &Groups) -> Result<(), String> {
    rq.app.store().save(g).map_err(|e| e.to_string())
}

/// Whether `m` is still there in `kept`: a system kept, a partition its
/// host still lists by ID and name. A system not connected keeps its
/// members.
fn present(kept: &[Kept], m: &Member) -> bool {
    let Some(k) = kept.iter().find(|k| k.name == m.system) else {
        return false;
    };
    match m.partition {
        None => true,
        Some(_) if k.link != mgmtproto::Link::Connected => true,
        Some(id) => k.partitions.iter().any(|p| p.id == id && p.name == m.name),
    }
}

/// Drop the members that went; save when any did.
fn prune(rq: &Rq<'_>, kept: &[Kept], mut g: Groups) -> Groups {
    let before = g.clone();
    for grp in &mut g.groups {
        grp.members.retain(|m| present(kept, m));
    }
    if g != before {
        let _ = save(rq, &g);
        rq.audit("custom groups", "members gone from their hosts removed");
    }
    g
}

/// A group's row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    /// Its name.
    pub name: String,
    /// What it is for.
    pub description: String,
    /// The systems it reaches.
    pub systems: usize,
    /// The partitions it reaches.
    pub partitions: usize,
    /// Who made it.
    pub owner: String,
}

/// The list.
pub fn page(ctx: &Ctx, rows: &[Row], messages: Vec<Message>) -> Page {
    let list = List {
        cols: vec![
            Col::plain("Actions"),
            Col::sorted("Group", "g"),
            Col::plain("Description"),
            Col::plain("Managed systems"),
            Col::plain("Partitions"),
            Col::plain("Owner"),
        ],
        rows: rows
            .iter()
            .map(|r| {
                let n = qsweb::http::form::encode(&r.name);
                vec![
                    Cell::act(format!(
                        "<a href=\"{}\">Open</a> | <a href=\"{}\">Edit</a> | <a href=\"{}\">Delete</a>",
                        esc(&ctx.url(&format!("/group/{n}"))),
                        esc(&ctx.url(&format!("/groups/create?g={n}"))),
                        esc(&ctx.url(&format!("/confirm/group-delete?g={n}")))
                    )),
                    Cell::mono(&r.name),
                    Cell::text(&r.description),
                    Cell::num(&r.systems.to_string()),
                    Cell::num(&r.partitions.to_string()),
                    Cell::text(&r.owner),
                ]
            })
            .collect(),
        empty: "No custom groups. Use Create group to make one.".into(),
        keep: Vec::new(),
    };
    let body = format!(
        "{}<form method=\"post\" action=\"{}\">{}<div class=\"buttons\"><input type=\"submit\" value=\"Create group...\"></div></form>",
        list_table(&list),
        esc(&ctx.url("/groups/create")),
        ctx.csrf_field()
    );
    Page {
        title: "Custom groups".into(),
        crumb: vec!["Systems Management".into(), "Custom groups".into()],
        h1: "Custom groups".into(),
        messages,
        body,
        help: "groups".into(),
        nav: Some(("Systems Management", Some("/groups"))),
        ..Page::default()
    }
}

/// The systems a group reaches and its rows' partitions.
fn reach(kept: &[Kept], g: &Group) -> (usize, usize) {
    let members = members_of(kept, g);
    let systems = members.len();
    let parts = members.iter().map(Kept::partition_count).sum();
    (systems, parts)
}

/// `kept` cut to `g`'s members: each system whole, or with only its
/// member partitions.
fn members_of(kept: &[Kept], g: &Group) -> Vec<Kept> {
    kept.iter()
        .filter_map(|k| {
            let mine: Vec<&Member> = g.members.iter().filter(|m| m.system == k.name).collect();
            if mine.is_empty() {
                return None;
            }
            if mine.iter().any(|m| m.partition.is_none()) {
                return Some(k.clone());
            }
            let mut k = k.clone();
            k.partitions.retain(|p| {
                mine.iter()
                    .any(|m| m.partition == Some(p.id) && m.name == p.name)
            });
            Some(k)
        })
        .collect()
}

/// `GET /groups?sort=`.
pub fn route(rq: &Rq<'_>) -> Response {
    let kept = crate::systems::systems(rq).unwrap_or_default();
    let g = prune(rq, &kept, load(rq));
    let mut rows: Vec<Row> = g
        .groups
        .iter()
        .map(|grp| {
            let (systems, partitions) = reach(&kept, grp);
            Row {
                name: grp.name.clone(),
                description: grp.description.clone(),
                systems,
                partitions,
                owner: grp.owner.clone(),
            }
        })
        .collect();
    if rq.q("sort") == "g" {
        rows.sort_by(|a, b| a.name.cmp(&b.name));
    }
    rq.page(page(&rq.ctx, &rows, Vec::new()))
}

/// A row of the editor's members.
#[derive(Clone, Debug, Default)]
pub struct Candidate {
    /// Its name.
    pub name: String,
    /// `Managed system (standalone)`, `Partition`.
    pub kind: String,
    /// The partition's system; empty for a system.
    pub system: String,
    /// Its status word.
    pub status: String,
    /// In the group.
    pub included: bool,
}

/// The editor.
#[derive(Clone, Debug, Default)]
pub struct Editor {
    /// The group's name.
    pub name: String,
    /// What it is for.
    pub description: String,
    /// The candidates.
    pub candidates: Vec<Candidate>,
    /// It is a group being changed, not made.
    pub editing: bool,
    /// The messages.
    pub messages: Vec<Message>,
}

/// The editor's page.
pub fn editor(ctx: &Ctx, v: &Editor) -> Page {
    let rows = v
        .candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let good = crate::partitions::list::good(&c.status).or(match c.status.as_str() {
                "Operating" => Some(true),
                _ => None,
            });
            vec![
                Cell::html(format!(
                    "<input type=\"checkbox\" name=\"m{}\"{}>",
                    i + 1,
                    if c.included { " checked" } else { "" }
                )),
                Cell::mono(&c.name),
                Cell::text(&c.kind),
                Cell::mono(&c.system),
                Cell::html(status(&c.status, good)),
            ]
        })
        .collect();
    let list = List {
        cols: ["Include", "Name", "Kind", "Managed system", "Status"]
            .iter()
            .map(|c| Col::plain(c))
            .collect(),
        rows,
        empty: "There are no managed systems.".into(),
        keep: Vec::new(),
    };
    let mut body = format!(
        "<form method=\"post\">{}<table class=\"props\"><tr><th>Group name <span class=\"req\">*</span></th><td><input type=\"text\" name=\"g\" value=\"{}\" size=\"20\"></td></tr><tr><th>Description</th><td><input type=\"text\" name=\"d\" value=\"{}\" size=\"40\"></td></tr></table><h3>Members</h3>{}",
        ctx.csrf_field(),
        esc(&v.name),
        esc(&v.description),
        list_table(&list)
    );
    if v.editing {
        let _ = write!(body, "{}", qsweb::partials::hidden("was", &v.name));
    }
    body.push_str("<div class=\"buttons\"><input type=\"submit\" name=\"ok\" value=\"OK\"><input type=\"submit\" name=\"cancel\" value=\"Cancel\"></div></form>");
    Page {
        title: "Create custom group".into(),
        crumb: vec![
            "Systems Management".into(),
            "Custom groups".into(),
            if v.editing { "Edit" } else { "Create" }.into(),
        ],
        h1: if v.editing {
            format!("Custom group: {}", v.name)
        } else {
            "Create custom group".into()
        },
        messages: v.messages.clone(),
        body,
        help: "groups".into(),
        nav: Some(("Systems Management", Some("/groups"))),
        ..Page::default()
    }
}

/// Every system and partition, the group's members ticked.
fn candidates(kept: &[Kept], g: &Group) -> Vec<(Candidate, Member)> {
    let mut out = Vec::new();
    for k in kept {
        let kind = if k.kind == SystemKind::Standalone {
            "Managed system (standalone)"
        } else {
            "Managed system (QS/VS host)"
        };
        let m = Member {
            system: k.name.clone(),
            partition: None,
            name: String::new(),
        };
        out.push((
            Candidate {
                name: k.name.clone(),
                kind: kind.into(),
                system: String::new(),
                status: k.status().into(),
                included: g.members.contains(&m),
            },
            m,
        ));
    }
    for k in kept.iter().filter(|k| k.kind == SystemKind::Host) {
        for p in &k.partitions {
            let m = Member {
                system: k.name.clone(),
                partition: Some(p.id),
                name: p.name.clone(),
            };
            out.push((
                Candidate {
                    name: p.name.clone(),
                    kind: "Partition".into(),
                    system: k.name.clone(),
                    status: crate::partitions::list::state_word(p.state).into(),
                    included: g.members.contains(&m),
                },
                m,
            ));
        }
    }
    out
}

/// `GET`, `POST /groups/create?g=`: a group made, or one changed.
pub fn edit(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let kept = crate::systems::systems(rq).unwrap_or_default();
    let mut all = load(rq);
    let was = if rq.post() {
        f.str("was").to_string()
    } else {
        rq.q("g")
    };
    let had = all.groups.iter().find(|g| g.name == was).cloned();
    let mut messages = Vec::new();
    // The list's Create group... posts here: the empty form.
    if rq.post() && !f.has("ok") && !f.has("cancel") {
        let v = Editor {
            candidates: candidates(&kept, &Group::default())
                .into_iter()
                .map(|(c, _)| c)
                .collect(),
            ..Editor::default()
        };
        return rq.page(editor(&rq.ctx, &v));
    }
    if rq.post() && f.has("cancel") {
        return Response::redirect(&rq.ctx.url("/groups"));
    }
    if rq.post() {
        if let Some(no) = lacks(rq, Task::Activate) {
            return no;
        }
        let name = f.str("g").trim().to_uppercase();
        let members: Vec<Member> = candidates(&kept, &Group::default())
            .into_iter()
            .enumerate()
            .filter(|(i, _)| f.has(&format!("m{}", i + 1)))
            .map(|(_, (_, m))| m)
            .collect();
        let taken = all.groups.iter().any(|g| g.name == name && g.name != was);
        if taken {
            messages.push(qmcmsg::message(
                Kind::Error,
                &qmcmsg::GROUP_EXISTS,
                &[&name],
            ));
        } else {
            let g = Group {
                name: name.clone(),
                description: f.str("d").trim().to_string(),
                owner: had
                    .as_ref()
                    .map_or_else(|| rq.user().to_string(), |h| h.owner.clone()),
                members,
            };
            all.groups.retain(|x| x.name != was);
            all.groups.push(g);
            all.groups.sort_by(|a, b| a.name.cmp(&b.name));
            match save(rq, &all) {
                Ok(()) => {
                    rq.audit(
                        &format!("custom group {name}"),
                        if had.is_some() { "changed" } else { "created" },
                    );
                    return rq.done(
                        "/groups",
                        qmcmsg::message(Kind::Done, &qmcmsg::GROUP_SAVED, &[&name]),
                    );
                }
                Err(e) => messages.push(qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[&e])),
            }
        }
    }
    let g = had.clone().unwrap_or_default();
    let v = Editor {
        name: if rq.post() {
            f.str("g").to_string()
        } else {
            g.name.clone()
        },
        description: if rq.post() {
            f.str("d").to_string()
        } else {
            g.description.clone()
        },
        candidates: candidates(&kept, &g).into_iter().map(|(c, _)| c).collect(),
        editing: had.is_some(),
        messages,
    };
    rq.page(editor(&rq.ctx, &v))
}

/// The page of group `name`: the Partitions list of its members.
pub fn view_page(ctx: &Ctx, name: &str, v: &crate::partitions::list::Partitions) -> Page {
    let mut page = crate::partitions::list::page(ctx, v);
    page.title = format!("Custom group: {name}");
    page.h1 = format!("Custom group: {name}");
    page.crumb = vec![
        "Systems Management".into(),
        "Custom groups".into(),
        name.into(),
    ];
    page.desc = Some(format!(
        "Partitions that are members of group {name}. Actions apply to the selection."
    ));
    page.nav = Some(("Systems Management", Some("/groups")));
    page.body = format!(
        "<form method=\"post\">{}{}</form>{}",
        ctx.csrf_field(),
        crate::partitions::list::table(ctx, v),
        paging(&v.paging)
    );
    page
}

/// `GET`, `POST /group/<name>`.
pub fn view(rq: &Rq<'_>, name: &str) -> Response {
    let kept = crate::systems::systems(rq).unwrap_or_default();
    let all = prune(rq, &kept, load(rq));
    let Some(g) = all.groups.iter().find(|g| g.name == name) else {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    };
    let members = members_of(&kept, g);
    let (mut v, _) = crate::partitions::list::view(rq, &members, Vec::new());
    v.group = Some(g.name.clone());
    if rq.post() {
        return crate::partitions::list::post(rq, &format!("/group/{name}"), &v);
    }
    rq.page(view_page(&rq.ctx, &g.name, &v))
}

/// `GET`, `POST /confirm/group-delete?g=`.
pub fn delete(rq: &Rq<'_>) -> Response {
    let f = rq.req.form();
    let name = if rq.post() {
        f.str("g").to_string()
    } else {
        rq.q("g")
    };
    let mut all = load(rq);
    if !all.groups.iter().any(|g| g.name == name) {
        return crate::app::routes::error(rq, 404, &qmcmsg::NOT_FOUND, &[&rq.req.path]);
    }
    if let Some(no) = lacks(rq, Task::Activate) {
        return no;
    }
    if rq.post() {
        if f.get("ok").is_none() {
            return Response::redirect(&rq.ctx.url("/groups"));
        }
        all.groups.retain(|g| g.name != name);
        return match save(rq, &all) {
            Ok(()) => {
                rq.audit(&format!("custom group {name}"), "deleted");
                rq.done(
                    "/groups",
                    qmcmsg::message(Kind::Done, &qmcmsg::GROUP_DELETED, &[&name]),
                )
            }
            Err(e) => rq.done(
                "/groups",
                qmcmsg::message(Kind::Error, &qmcmsg::NOT_VALID, &[&e]),
            ),
        };
    }
    let body = qsweb::pages::confirm_body(
        &rq.ctx,
        "The group is deleted; its systems and partitions are not changed. Press OK to delete it, or Cancel to return without it.",
        &format!(
            "<table class=\"props\" cellspacing=\"0\"><tr><th>Group</th><td>{}</td></tr></table>",
            qsweb::partials::mono(&name)
        ),
        &qsweb::partials::hidden("g", &name),
    );
    rq.page(Page {
        title: "Confirm Delete Group".into(),
        crumb: vec![
            "Systems Management".into(),
            "Custom groups".into(),
            name.clone(),
        ],
        h1: "Confirm Delete Group".into(),
        body,
        help: "groups".into(),
        nav: Some(("Systems Management", Some("/groups"))),
        ..Page::default()
    })
}
