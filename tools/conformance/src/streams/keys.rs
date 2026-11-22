// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The test keys of the three product streams: each the SHA-256 of a
//! phrase naming it, so every build and the harness agree on them with
//! nothing kept. A test image trusts them; no release image does (the
//! build gives a release image the keys `QSMC_STREAM_KEYS` names).

use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

/// The streams, by name.
pub const STREAMS: [&str; 3] = ["qsmc", "qsvs", "qs400"];

/// The test key of stream `name`.
pub fn test_key(name: &str) -> SigningKey {
    let seed: [u8; 32] = Sha256::digest(format!("QS/MC test stream key {name}").as_bytes()).into();
    SigningKey::from_bytes(&seed)
}

/// Another key, which no image trusts: an index signed by it is refused.
pub fn stranger() -> SigningKey {
    test_key("not a stream of the console's")
}

/// The keys' file of stream `name` as the image carries it: the public
/// half in hexadecimal, one line.
pub fn public_file(key: &SigningKey) -> String {
    format!("{}\n", hex::encode(key.verifying_key().to_bytes()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn each_stream_has_a_key_of_its_own_and_the_stranger_none() {
        let k: Vec<[u8; 32]> = super::STREAMS
            .iter()
            .map(|s| super::test_key(s).verifying_key().to_bytes())
            .collect();
        assert!(k[0] != k[1] && k[1] != k[2] && k[0] != k[2]);
        assert!(!k.contains(&super::stranger().verifying_key().to_bytes()));
        assert_eq!(
            super::test_key("qsmc").to_bytes(),
            super::test_key("qsmc").to_bytes()
        );
        assert_eq!(super::public_file(&super::test_key("qsvs")).len(), 65);
    }
}
