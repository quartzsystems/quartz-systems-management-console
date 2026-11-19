// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! QS/MC's words for what the processor has of what the cryptography of
//! QSI and the web server needs. The features and the check are
//! `base/processor`'s.

/// The line PID 1 prints for what is lacking.
pub fn line(lacking: &[&str]) -> String {
    if lacking.is_empty() {
        let names: Vec<&str> = processor::NEEDED.iter().map(|(_, name)| *name).collect();
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

/// The status screen's processor fact for what is lacking.
pub fn fact(lacking: &[&str]) -> String {
    if lacking.is_empty() {
        "has what the cryptography needs".to_string()
    } else {
        format!(
            "lacks {}: the web server and QSI cannot run on it",
            lacking.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fact_says_what_is_lacking() {
        assert_eq!(fact(&[]), "has what the cryptography needs");
        assert_eq!(
            fact(&["AES"]),
            "lacks AES: the web server and QSI cannot run on it"
        );
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
