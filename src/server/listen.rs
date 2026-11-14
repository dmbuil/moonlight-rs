//! The HTTPS listener: `[server] tls_cert_file` / `tls_key_file`.
//!
//! Moonlight terminates TLS itself, so a `reencrypt` Route (OpenShift's service CA) or any other
//! proxy can keep the last hop encrypted. The certificate is renewed behind the process's back
//! (the service CA rewrites the mounted Secret and restarts nothing), so [`Tls`] re-reads the two
//! files every [`RELOAD_EVERY`] and swaps in a changed pair once it is valid. New connections get
//! the new certificate; open ones keep the one they were made with.
//!
//! Rules kept here: errors name the file at fault and never include key bytes; an invalid pair
//! never replaces a working one; the provider is aws-lc-rs, the one reqwest already uses.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use rustls::ServerConfig;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// How often the certificate files are checked for a renewal.
pub const RELOAD_EVERY: Duration = Duration::from_secs(60);
/// A client that hasn't finished the handshake by then is dropped, so idle sockets can't pile up.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Pause after a failed `accept` (e.g. `EMFILE`), instead of spinning on it.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);

/// The certificate and key Moonlight serves, and where they come from.
pub struct Tls {
    /// `[server] tls_cert_file`: PEM chain, leaf first.
    cert_file: PathBuf,
    /// `[server] tls_key_file`: PEM private key.
    key_file: PathBuf,
    /// The rustls crypto provider the key is loaded with.
    provider: Arc<CryptoProvider>,
    /// Shared with the `ServerConfig`; a reload swaps the pair inside it.
    current: Arc<Resolver>,
}

/// Hands rustls the current pair on every handshake, so the `ServerConfig` is built only once.
struct Resolver(RwLock<Loaded>);

/// The pair in use.
struct Loaded {
    /// The chain and its signing key, as rustls takes them.
    key: Arc<CertifiedKey>,
    /// SHA-256 of both files as read: tells a renewal from an unchanged pair without keeping the
    /// key's bytes around for the comparison.
    digest: [u8; 32],
}

impl std::fmt::Debug for Resolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Resolver")
    }
}

impl ResolvesServerCert for Resolver {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.0.read().unwrap_or_else(PoisonError::into_inner).key.clone())
    }
}

/// Reads and checks a pair: a PEM chain (leaf first) and a PKCS#8, PKCS#1 or SEC1 key that
/// matches the leaf. Returns it with the digest of both files.
fn read_pair(cert_file: &Path, key_file: &Path, provider: &CryptoProvider) -> Result<Loaded, String> {
    let cert_pem = std::fs::read(cert_file).map_err(|e| format!("{}: {e}", cert_file.display()))?;
    let key_pem = Zeroizing::new(std::fs::read(key_file).map_err(|e| format!("{}: {e}", key_file.display()))?);
    let chain = CertificateDer::pem_slice_iter(&cert_pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}: not a PEM certificate chain ({e})", cert_file.display()))?;
    if chain.is_empty() {
        return Err(format!("{}: no PEM certificate in it", cert_file.display()));
    }
    // The parser's error isn't shown: it may quote a line of the key file.
    let key = PrivateKeyDer::from_pem_slice(&key_pem).map_err(|_| {
        format!(
            "{}: no PEM private key (PKCS#8, PKCS#1 or SEC1) in it",
            key_file.display()
        )
    })?;
    // Checks that the key belongs to the leaf; rustls's errors carry no key material.
    let key = CertifiedKey::from_der(chain, key, provider)
        .map_err(|e| format!("{}: not the key of {} ({e})", key_file.display(), cert_file.display()))?;
    let mut h = Sha256::new();
    h.update(&cert_pem);
    h.update([0]);
    h.update(&*key_pem);
    Ok(Loaded {
        key: Arc::new(key),
        digest: h.finalize().into(),
    })
}

impl Tls {
    /// Loads the pair once at start-up; an error stops Moonlight (`configuration error: …`).
    pub fn load(cert_file: &Path, key_file: &Path) -> Result<Tls, String> {
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let loaded = read_pair(cert_file, key_file, &provider)?;
        Ok(Tls {
            cert_file: cert_file.into(),
            key_file: key_file.into(),
            provider,
            current: Arc::new(Resolver(RwLock::new(loaded))),
        })
    }

    /// Re-reads the files: `Ok(true)` when a changed, valid pair replaced the current one,
    /// `Ok(false)` when they are unchanged, and an error (the current pair stays) when the new
    /// files don't load, e.g. caught half-written or with a key that doesn't match.
    pub fn reload_if_changed(&self) -> Result<bool, String> {
        let next = read_pair(&self.cert_file, &self.key_file, &self.provider)?;
        let mut cur = self.current.0.write().unwrap_or_else(PoisonError::into_inner);
        if cur.digest == next.digest {
            return Ok(false);
        }
        *cur = next;
        Ok(true)
    }

    /// The leaf certificate being served now.
    pub fn leaf(&self) -> CertificateDer<'static> {
        let cur = self.current.0.read().unwrap_or_else(PoisonError::into_inner);
        cur.key.cert[0].clone()
    }

    /// TLS 1.2 and 1.3 with rustls's safe defaults, no client certificates, ALPN `h2` and
    /// `http/1.1`. The certificate comes from [`Resolver`], so it follows reloads.
    fn server_config(&self) -> Result<Arc<ServerConfig>, String> {
        let mut config = ServerConfig::builder_with_provider(self.provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(|e| format!("TLS: {e}"))?
            .with_no_client_auth()
            .with_cert_resolver(self.current.clone());
        config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        Ok(Arc::new(config))
    }

    /// Checks the files every [`RELOAD_EVERY`] for as long as the process runs.
    fn spawn_reloader(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(RELOAD_EVERY);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            tick.tick().await; // the first tick is immediate: the pair was just loaded
            loop {
                tick.tick().await;
                match self.reload_if_changed() {
                    Ok(true) => tracing::info!("TLS certificate reloaded from {}", self.cert_file.display()),
                    Ok(false) => {}
                    Err(e) => tracing::warn!("TLS certificate not reloaded, still serving the previous one: {e}"),
                }
            }
        });
    }
}

/// Serves `app` over HTTPS on `listener` (HTTP/1.1 and HTTP/2, WebSocket upgrades included)
/// and starts the reload task. Runs until the process ends; only a bad TLS configuration
/// returns an error.
pub async fn serve(listener: tokio::net::TcpListener, app: axum::Router, tls: Arc<Tls>) -> Result<(), String> {
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use hyper_util::server::conn::auto::Builder;
    use hyper_util::service::TowerToHyperService;

    let acceptor = tokio_rustls::TlsAcceptor::from(tls.server_config()?);
    tls.spawn_reloader();
    loop {
        let (tcp, peer) = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("accept failed: {e}");
                tokio::time::sleep(ACCEPT_BACKOFF).await;
                continue;
            }
        };
        let _ = tcp.set_nodelay(true);
        let (acceptor, app) = (acceptor.clone(), app.clone());
        tokio::spawn(async move {
            // Scanners and TCP probes (the kubelet's included) fail here all day: `debug`.
            let stream = match tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(tcp)).await {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => return tracing::debug!(%peer, "TLS handshake failed: {e}"),
                Err(_) => return tracing::debug!(%peer, "TLS handshake timed out"),
            };
            if let Err(e) = Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(TokioIo::new(stream), TowerToHyperService::new(app))
                .await
            {
                tracing::debug!(%peer, "connection closed: {e}");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::ServerName;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/server/testdata/tls");

    fn fixture(name: &str) -> PathBuf {
        Path::new(DATA).join(name)
    }

    /// A temp dir holding `cert.pem` and `key.pem`, which the tests overwrite to simulate a
    /// renewal. Removed on drop.
    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str, cert: &str, key: &str) -> Self {
            let p = std::env::temp_dir().join(format!("moonlight-listen-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            let d = Dir(p);
            d.set(cert, key);
            d
        }
        fn cert(&self) -> PathBuf {
            self.0.join("cert.pem")
        }
        fn key(&self) -> PathBuf {
            self.0.join("key.pem")
        }
        fn set(&self, cert: &str, key: &str) {
            std::fs::copy(fixture(cert), self.cert()).unwrap();
            std::fs::copy(fixture(key), self.key()).unwrap();
        }
        fn load(&self) -> Result<Tls, String> {
            Tls::load(&self.cert(), &self.key())
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn leaf_of(name: &str) -> CertificateDer<'static> {
        CertificateDer::from_pem_file(fixture(name)).unwrap()
    }

    #[test]
    fn a_matching_pair_loads() {
        let tls = Tls::load(&fixture("a.pem"), &fixture("a.key")).unwrap();
        assert_eq!(tls.leaf(), leaf_of("a.pem"));
        assert!(tls.server_config().is_ok());
    }

    #[test]
    fn a_bad_pair_names_the_file_at_fault() {
        let fails = |cert: PathBuf, key: PathBuf, names: &Path| {
            let e = Tls::load(&cert, &key).err().expect("should fail");
            assert!(e.contains(&names.display().to_string()), "{e}");
            assert!(!e.contains("PRIVATE KEY"), "no key material: {e}");
        };
        // Another certificate's key.
        fails(fixture("a.pem"), fixture("b.key"), &fixture("b.key"));
        // A missing file.
        fails(fixture("a.pem"), fixture("nope.key"), &fixture("nope.key"));
        fails(fixture("nope.pem"), fixture("a.key"), &fixture("nope.pem"));
        // A file without a certificate.
        fails(fixture("README.md"), fixture("a.key"), &fixture("README.md"));
        fails(fixture("a.key"), fixture("a.key"), &fixture("a.key"));
        // A certificate given as the key.
        fails(fixture("a.pem"), fixture("a.pem"), &fixture("a.pem"));
    }

    #[test]
    fn reload_swaps_only_a_changed_valid_pair() {
        let d = Dir::new("reload", "a.pem", "a.key");
        let tls = d.load().unwrap();
        assert_eq!(tls.reload_if_changed(), Ok(false), "unchanged files");
        d.set("b.pem", "b.key");
        assert_eq!(tls.reload_if_changed(), Ok(true));
        assert_eq!(tls.leaf(), leaf_of("b.pem"));
        assert_eq!(tls.reload_if_changed(), Ok(false));
        // Caught mid-renewal: the new certificate, the old key.
        d.set("a.pem", "b.key");
        assert!(tls.reload_if_changed().is_err());
        assert_eq!(tls.leaf(), leaf_of("b.pem"), "a mismatched pair keeps the current one");
        std::fs::remove_file(d.key()).unwrap();
        assert!(tls.reload_if_changed().unwrap_err().contains("key.pem"));
        assert_eq!(tls.leaf(), leaf_of("b.pem"), "a missing key keeps the current one");
    }

    /// One HTTPS GET through a client that trusts only the test CA: the status line and the
    /// leaf the server presented.
    async fn get(addr: std::net::SocketAddr) -> (String, CertificateDer<'static>) {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(leaf_of("ca.pem")).unwrap();
        let config =
            rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::aws_lc_rs::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth();
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut s = tokio_rustls::TlsConnector::from(Arc::new(config))
            .connect(ServerName::try_from("localhost").unwrap(), tcp)
            .await
            .unwrap();
        let leaf = s.get_ref().1.peer_certificates().unwrap()[0].clone().into_owned();
        s.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut out = Vec::new();
        // A missing close_notify is the server's business; the response has arrived by then.
        let _ = s.read_to_end(&mut out).await;
        let out = String::from_utf8_lossy(&out).into_owned();
        (out.lines().next().unwrap_or_default().to_string(), leaf)
    }

    #[tokio::test]
    async fn serves_https_and_new_connections_get_the_reloaded_certificate() {
        let d = Dir::new("serve", "a.pem", "a.key");
        let tls = Arc::new(d.load().unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/", axum::routing::get(|| async { "moon" }));
        tokio::spawn(serve(listener, app, tls.clone()));

        let (status, leaf) = get(addr).await;
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert_eq!(leaf, leaf_of("a.pem"));

        d.set("b.pem", "b.key");
        assert_eq!(tls.reload_if_changed(), Ok(true));
        let (status, leaf) = get(addr).await;
        assert_eq!(status, "HTTP/1.1 200 OK");
        assert_eq!(leaf, leaf_of("b.pem"), "a new connection gets the renewed certificate");
    }
}
