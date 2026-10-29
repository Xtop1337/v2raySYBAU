//! Xray 26.10 removed `allowInsecure`. For servers marked "insecure" the app reads the server
//! certificate once before the core starts. A certificate that normal validation accepts needs
//! nothing (a CDN may serve a different one on every edge, so it must not be pinned); an
//! untrusted one (self-signed) gets its SHA-256 pinned (`pinnedPeerCertSha256`): the connection
//! works as before, and a certificate swapped by a middlebox later is still rejected by Xray.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::model::{Network, Profile, Security};

/// Accepts any certificate (it is only read), and records whether normal validation against
/// the public roots would have accepted it.
#[derive(Debug)]
struct ReadOnlyVerifier {
    provider: Arc<CryptoProvider>,
    webpki: Option<Arc<WebPkiServerVerifier>>,
    trusted: AtomicBool,
}

impl ServerCertVerifier for ReadOnlyVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let trusted =
            self.webpki.as_ref().is_some_and(|v| v.verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now).is_ok());
        self.trusted.store(trusted, Ordering::Relaxed);
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// The certificate `host:port` presents for `sni`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerCert {
    /// SHA-256 (lowercase hex) of the leaf certificate.
    pub sha256: String,
    /// Normal validation (public roots, name) accepts it.
    pub trusted: bool,
}

/// Reads the certificate presented by `host:port` for `sni`.
pub async fn peer_cert(host: &str, port: u16, sni: &str, alpn: &[String], timeout: Duration) -> Option<PeerCert> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let roots = Arc::new(rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() });
    let webpki = WebPkiServerVerifier::builder_with_provider(roots, provider.clone()).build().ok();
    let verifier = Arc::new(ReadOnlyVerifier { provider: provider.clone(), webpki, trusted: AtomicBool::new(false) });
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .ok()?
        .dangerous()
        .with_custom_certificate_verifier(verifier.clone())
        .with_no_client_auth();
    config.alpn_protocols =
        if alpn.is_empty() { vec![b"h2".to_vec(), b"http/1.1".to_vec()] } else { alpn.iter().map(|a| a.as_bytes().to_vec()).collect() };
    let name = ServerName::try_from(sni.to_string()).ok()?;
    let connect = async {
        let tcp = tokio::net::TcpStream::connect((host, port)).await.ok()?;
        let tls = tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp).await.ok()?;
        let leaf = tls.get_ref().1.peer_certificates()?.first()?.clone();
        Some(Sha256::digest(leaf.as_ref()).iter().map(|b| format!("{b:02x}")).collect::<String>())
    };
    let sha256 = tokio::time::timeout(timeout, connect).await.ok()??;
    Some(PeerCert { sha256, trusted: verifier.trusted.load(Ordering::Relaxed) })
}

/// SHA-256 (lowercase hex) of the leaf certificate presented by `host:port` for `sni`.
pub async fn leaf_sha256(host: &str, port: u16, sni: &str, alpn: &[String], timeout: Duration) -> Option<String> {
    peer_cert(host, port, sni, alpn, timeout).await.map(|c| c.sha256)
}

/// Profiles that relied on `allowInsecure` and now need a pinned certificate.
pub fn needs_pin(profile: &Profile, global_insecure: bool) -> bool {
    let tls = &profile.tls;
    !profile.protocol.needs_singbox()
        && tls.security == Security::Tls
        && (tls.allow_insecure || global_insecure)
        && tls.pinned_sha256.trim().is_empty()
        && !matches!(profile.transport.network, Network::Kcp | Network::Quic)
}

fn sni_of(profile: &Profile) -> String {
    let t = &profile.transport;
    if !profile.tls.sni.is_empty() {
        profile.tls.sni.clone()
    } else if !t.host.is_empty() && t.host.parse::<std::net::IpAddr>().is_err() {
        t.host.split(',').next().unwrap_or("").trim().to_string()
    } else {
        profile.address.clone()
    }
}

/// Pins the untrusted certificate of every profile that needs it. Returns the names of the
/// profiles whose certificate could not be read (they will fail certificate validation in Xray).
pub async fn pin_insecure(profiles: &mut [Profile], global_insecure: bool, timeout: Duration) -> Vec<String> {
    let targets: Vec<(usize, String, u16, String, Vec<String>)> = profiles
        .iter()
        .enumerate()
        .filter(|(_, p)| needs_pin(p, global_insecure))
        .map(|(i, p)| (i, p.address.clone(), p.port, sni_of(p), p.tls.alpn.clone()))
        .collect();
    let results: Vec<(usize, Option<PeerCert>)> = stream::iter(targets)
        .map(|(i, host, port, sni, alpn)| async move { (i, peer_cert(&host, port, &sni, &alpn, timeout).await) })
        .buffer_unordered(16)
        .collect()
        .await;
    let mut failed = Vec::new();
    for (i, cert) in results {
        match cert {
            Some(cert) if cert.trusted => {}
            Some(cert) => profiles[i].tls.pinned_sha256 = cert.sha256,
            None => failed.push(profiles[i].display_name()),
        }
    }
    failed
}
