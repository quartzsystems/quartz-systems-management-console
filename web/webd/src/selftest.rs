// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web server's first answer over TLS, asked of itself: a client
//! pinned to the certificate in use reads the stylesheet. Only then is
//! the server ready, and a slot on trial confirmed (`qsmcinit`'s
//! `slot::rule`).

use std::io::{BufRead, BufReader, Read, Write};
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme, StreamOwned};

use crate::product::CSS_PATH;

/// Accepts the one certificate whose SHA-256 it was given.
#[derive(Debug)]
pub struct Pinned {
    sha256: [u8; 32],
    provider: Arc<CryptoProvider>,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if x509kit::crypto::sha256(end_entity) == self.sha256 {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("not the pinned certificate".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A client that trusts only the certificate with `sha256`, as the
/// colon-separated hex the pages show.
pub fn pinned_client(sha256: &str) -> Result<Arc<ClientConfig>, String> {
    let bytes: Vec<u8> = sha256
        .split(':')
        .map(|h| u8::from_str_radix(h, 16))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let sha256: [u8; 32] = bytes.try_into().map_err(|_| "not a SHA-256")?;
    let provider = crate::tls::provider();
    let config = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned { sha256, provider }))
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Ask for the stylesheet over `sock` and say whether it came whole.
pub fn ask<S: Read + Write>(client: Arc<ClientConfig>, sock: S) -> Result<(), String> {
    let name = ServerName::try_from("qsmc.local").map_err(|e| e.to_string())?;
    let conn = ClientConnection::new(client, name).map_err(|e| e.to_string())?;
    let mut tls = StreamOwned::new(conn, sock);
    let req = format!("GET {CSS_PATH} HTTP/1.1\r\nHost: qsmc.local\r\nConnection: close\r\n\r\n");
    tls.write_all(req.as_bytes())
        .and_then(|()| tls.flush())
        .map_err(|e| format!("the request: {e}"))?;
    let mut reader = BufReader::new(tls);
    let mut status = String::new();
    reader
        .read_line(&mut status)
        .map_err(|e| format!("the answer: {e}"))?;
    if status.split(' ').nth(1) != Some("200") {
        return Err(format!("the answer was `{}`", status.trim_end()));
    }
    let mut rest = Vec::new();
    let _ = reader.read_to_end(&mut rest);
    if rest.ends_with(qsweb::CSS) {
        Ok(())
    } else {
        Err("the stylesheet did not come whole".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serve::{NoPages, Site};
    use crate::tls::{Certs, server_config};

    #[test]
    fn the_server_answers_a_client_pinned_to_it() {
        let dir = std::env::temp_dir().join(format!("webd-self-{}", std::process::id()));
        let (certs, _) = Certs::load_or_make(&dir, "QSMC000001", &[], 1_790_000_000).unwrap();
        let sha = certs.shown().sha256;
        let config = server_config(Arc::new(certs)).unwrap();
        let (a, b) = std::os::unix::net::UnixStream::pair().unwrap();
        let served = std::thread::spawn(move || {
            let conn = rustls::ServerConnection::new(config).unwrap();
            let mut tls = StreamOwned::new(conn, b);
            let peer = qsweb::http::Peer {
                addr: "127.0.0.1:1".parse().unwrap(),
                secure: true,
            };
            let site = Site {
                pages: Arc::new(NoPages),
            };
            qsweb::http::serve(&mut tls, &peer, &site);
            tls.conn.send_close_notify();
            let _ = tls.conn.complete_io(&mut tls.sock);
        });
        assert_eq!(ask(pinned_client(&sha).unwrap(), a), Ok(()));
        served.join().unwrap();
        let wrong = "00:".repeat(31) + "00";
        let (c, _d) = std::os::unix::net::UnixStream::pair().unwrap();
        c.set_read_timeout(Some(std::time::Duration::from_millis(100)))
            .unwrap();
        assert!(ask(pinned_client(&wrong).unwrap(), c).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
