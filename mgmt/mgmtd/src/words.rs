// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The console's own messages as `mgmtd` gives them: a catalogue entry
//! (`web/msgs`) with its values.

use mgmtproto::{Note, Refusal};

/// `def`'s text with `&1`, `&2`, ... replaced by `data`.
pub fn text(def: &qmcmsg::Def, data: &[&str]) -> String {
    let mut t = def.text.to_string();
    for (i, d) in data.iter().enumerate().rev() {
        t = t.replace(&format!("&{}", i + 1), d);
    }
    t
}

/// A refusal of the console's message `def`.
pub fn qmc(def: &qmcmsg::Def, data: &[&str]) -> Refusal {
    Refusal {
        main: Note {
            msgid: def.id.into(),
            text: text(def, data),
            help: String::new(),
        },
        also: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_go_in_their_places() {
        let r = qmc(&qmcmsg::NOT_CONNECTED, &["QSHQVS1", "it did not answer"]);
        assert_eq!(r.main.msgid, "QMC1106");
        assert_eq!(
            r.main.text,
            "Managed system QSHQVS1 is not connected: it did not answer."
        );
    }
}
