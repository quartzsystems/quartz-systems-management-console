// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The sign-on page (`signon.html`): `qsweb`'s, under QS/MC's brand,
//! with the console's notice in the sign-on box.

use qsweb::msg::Message;
use webd::product::PRODUCT;

/// What the sign-on page shows.
#[derive(Clone, Debug, Default)]
pub struct Signon {
    /// The console's name.
    pub console: String,
    /// The notice, from Language, locale and session; none when empty.
    pub notice: String,
    /// The user ID typed last, after a refusal.
    pub user: String,
    /// The message rows.
    pub messages: Vec<Message>,
}

/// The page.
pub fn render(v: &Signon) -> String {
    let notice = (!v.notice.is_empty()).then_some(v.notice.as_str());
    qsweb::frame::signon(&PRODUCT, &v.console, &v.user, &v.messages, notice)
}
