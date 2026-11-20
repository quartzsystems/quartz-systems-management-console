// Copyright (C), 2026 Quartz Systems. Some rights reserved. This work is
// licensed under the terms of the MIT license which can be found in the
// root directory of this project.

//! The web certificate and TLS. The certificate and key are PEM files
//! under [`release::WEB_DIR`]; the first start makes them (ECDSA P-256,
//! self-signed, `x509kit`) for the host name and the `open` addresses.
//! TLS 1.2 and 1.3 by rustls with the graviola provider and its default
//! suites. A certificate replaced is served to the next handshake; the
//! sessions already open keep theirs. This is not the console's QSI
//! identity, which stays as it is.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use rustls::ServerConfig;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use x509kit::bundle::{self, Bundle};
use x509kit::crypto::keys::PrivateKey;
use x509kit::x509::CertInfo;

/// Days a self-signed certificate is made valid for.
pub const SELF_SIGNED_DAYS: u32 = 365;

/// The certificate's file, the chain with the leaf first.
pub const CERT_FILE: &str = release::WEB_CERT_FILE;
/// The key's file, PKCS #8, readable by its owner only.
pub const KEY_FILE: &str = release::WEB_KEY_FILE;

/// The provider every TLS object here uses.
pub fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls_graviola::default_provider())
}

/// What a page shows of the certificate in use.
#[derive(Clone, Debug)]
pub struct Shown {
    /// The leaf's fields.
    pub info: CertInfo,
    /// The leaf's SHA-256, colon-separated upper-case hex.
    pub sha256: String,
    /// The leaf's SHA-1, the same form.
    pub sha1: String,
    /// Certificates in the chain, the leaf included.
    pub chain: usize,
}

/// `AA:BB:...` of `bytes`.
pub fn colons(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

impl Shown {
    fn of(chain: &[Vec<u8>]) -> Result<Self, String> {
        let leaf = chain.first().ok_or("no certificate")?;
        let info = CertInfo::from_der(leaf).map_err(|e| e.to_string())?;
        Ok(Self {
            info,
            sha256: colons(&x509kit::crypto::sha256(leaf)),
            sha1: colons(&x509kit::crypto::sha1(leaf)),
            chain: chain.len(),
        })
    }
}

/// The certificate in use, swapped whole when replaced.
#[derive(Debug)]
struct InUse {
    key: Arc<CertifiedKey>,
    shown: Shown,
}

/// The certificate the server presents.
#[derive(Debug)]
pub struct Certs {
    dir: PathBuf,
    now: RwLock<Arc<InUse>>,
    provider: Arc<CryptoProvider>,
}

impl ResolvesServerCert for Certs {
    fn resolve(&self, _hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.in_use().key.clone())
    }
}

fn certified(
    provider: &CryptoProvider,
    chain: &[Vec<u8>],
    key: &PrivateKey,
) -> Result<CertifiedKey, String> {
    let der = key.to_pkcs8_der().map_err(|e| e.to_string())?;
    let signing = provider
        .key_provider
        .load_private_key(PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(der.to_vec())))
        .map_err(|e| format!("the key: {e}"))?;
    let chain = chain.iter().cloned().map(CertificateDer::from).collect();
    Ok(CertifiedKey::new(chain, signing))
}

/// Write `text` to `path` through a new file, `0o600` when `secret`.
fn write_file(path: &Path, text: &str, secret: bool) -> Result<(), String> {
    use std::io::Write;
    let fresh = path.with_extension("new");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if secret { 0o600 } else { 0o644 });
    }
    let mut file = options
        .open(&fresh)
        .map_err(|e| format!("{}: {e}", fresh.display()))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", fresh.display()))?;
    std::fs::rename(&fresh, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The bundle an upload holds: PEM text (its key opened with
/// `password` when it is encrypted), or PKCS #12 with its password.
pub fn read_bundle(data: &[u8], password: &str) -> Result<Bundle, String> {
    if data.is_empty() {
        return Err("no file was chosen".into());
    }
    match std::str::from_utf8(data) {
        Ok(text) if text.contains("-----BEGIN") => {
            bundle::read_pem(text, Some(password).filter(|p| !p.is_empty()))
                .map_err(|e| e.to_string())
        }
        _ => bundle::read_pkcs12(data, password).map_err(|e| e.to_string()),
    }
}

/// A bundle's chain and key, both there and matching.
pub fn checked(bundle: Bundle) -> Result<(Vec<Vec<u8>>, PrivateKey), String> {
    let key = bundle.key.ok_or("there is no private key")?;
    if bundle.key_matches != Some(true) {
        return Err("the private key does not belong to the certificate".into());
    }
    if bundle.chain.is_empty() {
        return Err("there is no certificate".into());
    }
    Ok((bundle.chain.into_iter().map(|c| c.der).collect(), key))
}

impl Certs {
    /// The certificate in `dir`; a new self-signed one for `host` and
    /// `addresses` when there is none or it cannot be read. Says which.
    pub fn load_or_make(
        dir: &Path,
        host: &str,
        addresses: &[String],
        now: u64,
    ) -> Result<(Self, bool), String> {
        let provider = provider();
        let read = std::fs::read_to_string(dir.join(CERT_FILE))
            .and_then(|c| Ok(c + &std::fs::read_to_string(dir.join(KEY_FILE))?))
            .map_err(|e| e.to_string())
            .and_then(|text| bundle::read_pem(&text, None).map_err(|e| e.to_string()))
            .and_then(checked);
        let (chain, key, made) = if let Ok((chain, key)) = read {
            (chain, key, false)
        } else {
            let (chain, key) = Self::make_files(dir, host, addresses, now)?;
            (chain, key, true)
        };
        let in_use = InUse {
            key: Arc::new(certified(&provider, &chain, &key)?),
            shown: Shown::of(&chain)?,
        };
        Ok((
            Self {
                dir: dir.to_path_buf(),
                now: RwLock::new(Arc::new(in_use)),
                provider,
            },
            made,
        ))
    }

    fn make_files(
        dir: &Path,
        host: &str,
        addresses: &[String],
        now: u64,
    ) -> Result<(Vec<Vec<u8>>, PrivateKey), String> {
        let mut names: Vec<&str> = vec![host];
        names.extend(addresses.iter().map(String::as_str));
        let id =
            x509kit::server::self_signed(host, &names, now.saturating_sub(60), SELF_SIGNED_DAYS)
                .map_err(|e| format!("the certificate was not made: {e}"))?;
        Self::write(dir, std::slice::from_ref(&id.cert), &id.key)?;
        Ok((vec![id.cert], id.key))
    }

    fn write(dir: &Path, chain: &[Vec<u8>], key: &PrivateKey) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let key_der = key.to_pkcs8_der().map_err(|e| e.to_string())?;
        let key_pem = zeroize::Zeroizing::new(x509kit::pem::encode("PRIVATE KEY", &key_der));
        write_file(&dir.join(KEY_FILE), &key_pem, true)?;
        let certs: String = chain
            .iter()
            .map(|c| x509kit::pem::encode("CERTIFICATE", c))
            .collect();
        write_file(&dir.join(CERT_FILE), &certs, false)
    }

    fn in_use(&self) -> Arc<InUse> {
        self.now
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// What the pages show of the certificate in use.
    pub fn shown(&self) -> Shown {
        self.in_use().shown.clone()
    }

    /// The PEM of the chain's last certificate: the authority a browser
    /// is told to trust (the certificate itself when self-signed).
    pub fn authority_pem(&self) -> String {
        self.in_use()
            .key
            .cert
            .last()
            .map(|c| x509kit::pem::encode("CERTIFICATE", c))
            .unwrap_or_default()
    }

    fn take(&self, chain: &[Vec<u8>], key: &PrivateKey) -> Result<(), String> {
        let in_use = InUse {
            key: Arc::new(certified(&self.provider, chain, key)?),
            shown: Shown::of(chain)?,
        };
        Self::write(&self.dir, chain, key)?;
        *self.now.write().unwrap_or_else(PoisonError::into_inner) = Arc::new(in_use);
        Ok(())
    }

    /// Replace the certificate with a new self-signed one.
    pub fn renew(&self, host: &str, addresses: &[String], now: u64) -> Result<Shown, String> {
        let mut names: Vec<&str> = vec![host];
        names.extend(addresses.iter().map(String::as_str));
        let id =
            x509kit::server::self_signed(host, &names, now.saturating_sub(60), SELF_SIGNED_DAYS)
                .map_err(|e| format!("the certificate was not made: {e}"))?;
        self.take(&[id.cert], &id.key)?;
        Ok(self.shown())
    }

    /// Replace the certificate with an imported chain and key: PEM, or
    /// PKCS #12 with its password.
    pub fn import(&self, bundle: Bundle) -> Result<Shown, String> {
        let (chain, key) = checked(bundle)?;
        self.take(&chain, &key)?;
        Ok(self.shown())
    }
}

/// The server's TLS configuration over `certs`.
pub fn server_config(certs: Arc<Certs>) -> Result<Arc<ServerConfig>, String> {
    let config = ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_cert_resolver(certs);
    Ok(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("webd-tls-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn made_once_then_read_and_renewed() {
        let dir = scratch("made");
        let now = 1_790_000_000;
        let addresses = vec!["192.0.2.7".to_string()];
        let (certs, was_made) = Certs::load_or_make(&dir, "QSMC0A1B2C", &addresses, now).unwrap();
        assert!(was_made);
        let first = certs.shown();
        assert_eq!(first.info.common_name.as_deref(), Some("QSMC0A1B2C"));
        assert!(first.info.san.contains(&"192.0.2.7".to_string()));
        assert_eq!(first.sha256.len(), 32 * 3 - 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(KEY_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let (again, made) = Certs::load_or_make(&dir, "OTHER", &[], now).unwrap();
        assert!(!made);
        assert_eq!(again.shown().sha256, first.sha256);
        let renewed = again.renew("QSMC0A1B2C", &addresses, now).unwrap();
        assert_ne!(renewed.sha256, first.sha256);
        assert!(server_config(Arc::new(again)).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_key_that_does_not_match_is_refused() {
        let dir = scratch("mismatch");
        let now = 1_790_000_000;
        let (certs, _) = Certs::load_or_make(&dir, "A", &[], now).unwrap();
        let one = x509kit::server::self_signed("one", &["one"], now, 1).unwrap();
        let two = x509kit::server::self_signed("two", &["two"], now, 1).unwrap();
        let pem = one.cert_pem() + &two.key_pem().unwrap();
        let bundle = bundle::read_pem(&pem, None).unwrap();
        let before = certs.shown().sha256;
        assert!(certs.import(bundle).is_err());
        assert_eq!(certs.shown().sha256, before);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
