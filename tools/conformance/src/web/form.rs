// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! A page's forms as a browser without script sees them: each form's
//! action and method, the fields it would send (text and hidden inputs,
//! checked boxes and radios, each select's chosen option) and its submit
//! buttons, including those outside it that name it with `form=`.

use qsweb::golden::{Tok, tokens};

/// One form of a page.
#[derive(Clone, Debug, Default)]
pub struct Form {
    /// Its `id`, when it has one.
    pub id: String,
    /// Where it posts: the page's own address when it names none.
    pub action: String,
    /// `post` or `get`.
    pub method: String,
    /// `multipart/form-data` for an upload.
    pub enctype: String,
    /// What it sends, in order.
    pub fields: Vec<(String, String)>,
    /// Its submit buttons: name (empty for none), value, `formaction`.
    pub submits: Vec<(String, String, String)>,
    /// It has a file input.
    pub file: bool,
}

impl Form {
    /// Whether it carries the session's form token.
    pub fn has_csrf(&self) -> bool {
        self.fields.iter().any(|(n, _)| n == "csrf")
    }

    /// The value of field `name`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Whether a submit button is named `name`.
    pub fn submits_as(&self, name: &str) -> bool {
        self.submits.iter().any(|(n, _, _)| n == name)
    }

    /// Its fields with `set` applied: a value replaces the field's (or is
    /// added), an empty name's entry is ignored, and a value of
    /// [`OFF`] takes the field away (a box unticked).
    pub fn with(&self, set: &[(&str, &str)]) -> Vec<(String, String)> {
        let mut out = self.fields.clone();
        for (n, v) in set {
            if *v == OFF {
                out.retain(|(k, _)| k != n);
            } else if let Some(f) = out.iter_mut().find(|(k, _)| k == n) {
                f.1 = (*v).to_string();
            } else {
                out.push(((*n).to_string(), (*v).to_string()));
            }
        }
        out
    }
}

/// The value that unticks a box in [`Form::with`].
pub const OFF: &str = "\u{0}off";

/// An attribute's value with the entities the pages write decoded.
fn attr(a: &Attrs, name: &str) -> Option<String> {
    a.get(name).map(|v| {
        v.replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
    })
}

/// A `<select>` being read: its name, its first and chosen options, the
/// form it is in.
struct Select {
    name: String,
    first: Option<String>,
    chosen: Option<String>,
    form: Option<usize>,
}

/// An `<option>` being read: its value, whether it is chosen, its text.
struct Opt {
    value: Option<String>,
    chosen: bool,
    text: String,
}

type Attrs = std::collections::BTreeMap<String, String>;

/// The forms of a page as its tokens are read.
#[derive(Default)]
struct Reader {
    out: Vec<Form>,
    /// Buttons outside any form that name one, kept for after.
    outside: Vec<(String, (String, String, String))>,
    current: Option<usize>,
    select: Option<Select>,
    option: Option<Opt>,
}

impl Reader {
    fn input(&mut self, a: &Attrs) {
        if a.contains_key("disabled") {
            return;
        }
        let name = attr(a, "name").unwrap_or_default();
        let value = attr(a, "value");
        let kind = attr(a, "type").unwrap_or_else(|| "text".into());
        let owner = attr(a, "form");
        match kind.as_str() {
            "submit" | "image" => {
                let s = (
                    name,
                    value.unwrap_or_default(),
                    attr(a, "formaction").unwrap_or_default(),
                );
                match (owner, self.current) {
                    (Some(id), _) => self.outside.push((id, s)),
                    (None, Some(i)) => self.out[i].submits.push(s),
                    (None, None) => {}
                }
            }
            "file" => {
                if let Some(i) = self.current {
                    self.out[i].file = true;
                }
            }
            "checkbox" | "radio" => {
                if a.contains_key("checked")
                    && let Some(i) = self.current
                    && !name.is_empty()
                {
                    self.out[i]
                        .fields
                        .push((name, value.unwrap_or_else(|| "on".into())));
                }
            }
            _ => {
                let i = match owner {
                    Some(id) => self.out.iter().position(|f| f.id == id),
                    None => self.current,
                };
                if let Some(i) = i
                    && !name.is_empty()
                {
                    self.out[i].fields.push((name, value.unwrap_or_default()));
                }
            }
        }
    }

    fn token(&mut self, t: Tok, here: &str) {
        match t {
            Tok::Open(n, a) if n == "form" => {
                let action = attr(&a, "action").filter(|s| !s.is_empty());
                self.out.push(Form {
                    id: attr(&a, "id").unwrap_or_default(),
                    action: action.unwrap_or_else(|| here.to_string()),
                    method: attr(&a, "method").unwrap_or_else(|| "get".into()),
                    enctype: attr(&a, "enctype").unwrap_or_default(),
                    ..Form::default()
                });
                self.current = Some(self.out.len() - 1);
            }
            Tok::Close(n) if n == "form" => self.current = None,
            Tok::Open(n, a) if n == "input" => self.input(&a),
            Tok::Open(n, a) if n == "select" => {
                self.select = Some(Select {
                    name: attr(&a, "name").unwrap_or_default(),
                    first: None,
                    chosen: None,
                    form: self.current,
                });
            }
            Tok::Open(n, a) if n == "option" => {
                self.option = Some(Opt {
                    value: attr(&a, "value"),
                    chosen: a.contains_key("selected"),
                    text: String::new(),
                });
            }
            Tok::Text(x) => {
                if let Some(o) = self.option.as_mut() {
                    o.text.push_str(&x);
                }
            }
            Tok::Close(n) if n == "option" => {
                if let (Some(o), Some(s)) = (self.option.take(), self.select.as_mut()) {
                    let v = o.value.unwrap_or_else(|| o.text.trim().to_string());
                    if s.first.is_none() {
                        s.first = Some(v.clone());
                    }
                    if o.chosen && s.chosen.is_none() {
                        s.chosen = Some(v);
                    }
                }
            }
            Tok::Close(n) if n == "select" => {
                if let Some(s) = self.select.take()
                    && let Some(i) = s.form
                    && !s.name.is_empty()
                {
                    self.out[i]
                        .fields
                        .push((s.name, s.chosen.or(s.first).unwrap_or_default()));
                }
            }
            _ => {}
        }
    }
}

/// The forms of `html`, served at `here`.
pub fn forms(html: &str, here: &str) -> Vec<Form> {
    let mut r = Reader::default();
    for t in tokens(html) {
        r.token(t, here);
    }
    let mut out = r.out;
    for (id, s) in r.outside {
        if let Some(f) = out.iter_mut().find(|f| f.id == id) {
            f.submits.push(s);
        }
    }
    out
}
/// The form of `html` with a submit button named `submit`, or the
/// first form when none has one.
pub fn pick(html: &str, here: &str, submit: &str) -> Option<Form> {
    let all = forms(html, here);
    all.iter()
        .find(|f| f.submits_as(submit))
        .or_else(|| all.first())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_form_sends_what_a_browser_sends() {
        let html = "<form method=\"post\"><input type=\"hidden\" name=\"csrf\" value=\"t\">\
            <input type=\"text\" name=\"n\" value=\"a&amp;b\"><input type=\"checkbox\" name=\"c\" checked>\
            <input type=\"checkbox\" name=\"d\"><select name=\"s\"><option>One</option><option selected>Two</option></select>\
            <input type=\"radio\" name=\"r\" value=\"x\"><input type=\"radio\" name=\"r\" value=\"y\" checked>\
            <input type=\"submit\" name=\"ok\" value=\"OK\"></form>\
            <form id=\"f\" method=\"post\" action=\"/qsmc/x\"></form><input type=\"submit\" name=\"go\" form=\"f\">";
        let f = forms(html, "/qsmc/here");
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].action, "/qsmc/here");
        assert_eq!(
            f[0].fields,
            [
                ("csrf".to_string(), "t".to_string()),
                ("n".into(), "a&b".into()),
                ("c".into(), "on".into()),
                ("s".into(), "Two".into()),
                ("r".into(), "y".into()),
            ]
        );
        assert!(f[0].has_csrf() && f[0].submits_as("ok"));
        assert!(f[1].submits_as("go") && f[1].action == "/qsmc/x");
        let set = f[0].with(&[("c", OFF), ("n", "z"), ("new", "1")]);
        assert!(!set.iter().any(|(k, _)| k == "c"));
        assert!(
            set.contains(&("n".into(), "z".into())) && set.contains(&("new".into(), "1".into()))
        );
    }
}
