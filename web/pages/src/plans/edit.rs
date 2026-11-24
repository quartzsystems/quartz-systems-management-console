// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A plan's description and Save as template: a page the prototype has
//! no design of, built from the properties pattern. A template is the
//! plan without its host's name and serial number, its partitions'
//! serial numbers and MAC addresses (`docs/PLANS.md`).

use qsweb::frame::{Ctx, Page};

use qsweb::http::Response;
use qsweb::msg::{Kind, Message};
use qsweb::partials::{Button, Prop, PropGroup, props};

use super::{NAV, TITLE, down, issued, route_of};
use crate::app::routes::Rq;

/// What the page shows.
#[derive(Clone, Debug, Default)]
pub struct Edit {
    /// The plan.
    pub name: String,
    /// Its description, as typed.
    pub description: String,
    /// The template's name, as typed.
    pub template: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn page(ctx: &Ctx, v: &Edit) -> Page {
    let groups = [PropGroup {
        title: None,
        rows: vec![
            Prop::new("Plan", qsweb::partials::mono(&v.name)),
            Prop::new(
                "Description",
                qsweb::partials::input("d", &v.description, 50),
            ),
            Prop::new(
                "Template name",
                format!(
                    "{} <span class=\"small grey\">for Save as template: 1 to 32 of a-z, 0-9, -, _ and .</span>",
                    qsweb::partials::input("t", &v.template, 32)
                ),
            ),
        ],
    }];
    let body = props(
        ctx,
        &groups,
        &[
            Button::new("ok", "OK"),
            Button::new("template", "Save as template"),
            Button::new("cancel", "Cancel"),
        ],
    );
    Page {
        title: format!("System plan: {}", v.name),
        crumb: vec![NAV.into(), TITLE.into(), v.name.clone(), "Edit".into()],
        h1: format!("System plan: {}", v.name),
        desc: Some(format!(
            "Change the description of {}, or save it as a template.",
            v.name
        )),
        messages: v.messages.clone(),
        body,
        help: "plans".into(),
        nav: Some((NAV, Some("/plans"))),
        ..Page::default()
    }
}

/// `GET`, `POST /plan/<name>/edit`.
pub fn route(rq: &Rq<'_>, name: &str) -> Response {
    let plan = match super::view::read(rq, name) {
        Ok(Some(p)) => p,
        Ok(None) => return crate::app::routes::error(rq, 404, &qmcmsg::NO_SUCH_PLAN, &[name]),
        Err(m) => return rq.done("/plans", m),
    };
    let f = rq.req.form();
    let mut v = Edit {
        name: name.to_string(),
        description: plan.description.clone(),
        template: format!("{name}-template").chars().take(32).collect(),
        messages: Vec::new(),
    };
    if !rq.post() {
        return rq.page(page(&rq.ctx, &v));
    }
    if f.has("cancel") {
        return Response::redirect(&rq.ctx.url(&route_of(name, "")));
    }
    v.description = f.str("d").trim().to_string();
    v.template = f.str("t").trim().to_string();
    let mut sched = rq.app.console.sched();
    let user = rq.user().to_string();
    if f.has("template") {
        return match sched.template(name.to_string(), v.template.clone(), user) {
            Ok(Ok(e)) => rq.done(
                "/plans",
                qmcmsg::message(Kind::Done, &qmcmsg::TEMPLATE_SAVED, &[&e.name, name]),
            ),
            Ok(Err(i)) => {
                v.messages.push(issued(Kind::Error, &i));
                rq.page(page(&rq.ctx, &v))
            }
            Err(e) => {
                v.messages.push(down(&e));
                rq.page(page(&rq.ctx, &v))
            }
        };
    }
    match sched.describe_plan(name.to_string(), v.description.clone(), user) {
        Ok(Ok(())) => rq.done(
            &route_of(name, ""),
            qmcmsg::message(Kind::Done, &qmcmsg::PLAN_DESCRIBED, &[name]),
        ),
        Ok(Err(i)) => {
            v.messages.push(issued(Kind::Error, &i));
            rq.page(page(&rq.ctx, &v))
        }
        Err(e) => {
            v.messages.push(down(&e));
            rq.page(page(&rq.ctx, &v))
        }
    }
}
