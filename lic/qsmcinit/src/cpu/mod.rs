// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! What the cryptography of QSI and the web server needs of the
//! processor (QS/400's `docs/QSI.md`, section 12): graviola uses AVX2,
//! ADX, AES and PCLMULQDQ, and faults at its first use on a processor
//! without one of them. The same four QS/VS's PID 1 asks for, through
//! `qsilocal::processor` in its `qsi/local`: that crate is not in its
//! `base/`, so the check is here until it moves there
//! (`docs/OPEN_QUESTIONS.md` item 3).

/// Each feature needed: its `/proc/cpuinfo` flag and its name.
pub const NEEDED: [(&str, &str); 4] = [
    ("avx2", "AVX2"),
    ("adx", "ADX"),
    ("aes", "AES"),
    ("pclmulqdq", "PCLMULQDQ"),
];

/// The names of what this processor lacks; none when it has all.
pub fn lacking() -> Vec<&'static str> {
    #[cfg(target_arch = "x86_64")]
    {
        let has = [
            std::is_x86_feature_detected!("avx2"),
            std::is_x86_feature_detected!("adx"),
            std::is_x86_feature_detected!("aes"),
            std::is_x86_feature_detected!("pclmulqdq"),
        ];
        NEEDED
            .iter()
            .zip(has)
            .filter(|(_, has)| !has)
            .map(|((_, name), _)| *name)
            .collect()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        NEEDED.iter().map(|(_, name)| *name).collect()
    }
}

/// The line PID 1 prints for what is lacking.
pub fn line(lacking: &[&str]) -> String {
    if lacking.is_empty() {
        let names: Vec<&str> = NEEDED.iter().map(|(_, name)| *name).collect();
        format!(
            "Processor has what the cryptography needs: {}",
            names.join(", ")
        )
    } else {
        format!(
            "Processor lacks {}: the web server and QSI cannot run on it",
            lacking.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_are_qsi_s() {
        let flags: Vec<&str> = NEEDED.iter().map(|(flag, _)| *flag).collect();
        assert_eq!(flags, ["avx2", "adx", "aes", "pclmulqdq"]);
    }

    #[test]
    fn the_line_says_which() {
        assert_eq!(
            line(&[]),
            "Processor has what the cryptography needs: AVX2, ADX, AES, PCLMULQDQ"
        );
        assert_eq!(
            line(&["AVX2", "ADX"]),
            "Processor lacks AVX2, ADX: the web server and QSI cannot run on it"
        );
    }
}
