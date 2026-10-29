//! Xray 26.10 removed `allowInsecure`. For servers marked "insecure" (self-signed certificates)
//! the app reads the server certificate once before the core starts and pins its SHA-256
//! (`pinnedPeerCertSha256`): the connection works as before, and a certificate swapped by a
//! middlebox later is still rejected by Xray.

use std::sync::Arc;
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::model::{Network, Profile, Security};

/// Accepts any certificate: used only to read the certificate that is then pinned.
#[derive(Debug)]
struct ReadOnlyVerifier(Arc<CryptoProvider>);

impl ServerCertVerifier for ReadOnlyVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// SHA-256 (lowercase hex) of the leaf certificate presented by `host:port` for `sni`.
pub async fn leaf_sha256(host: &str, port: u16, sni: &str, alpn: &[String], timeout: Duration) -> Option<String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .ok()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(ReadOnlyVerifier(provider)))
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
    tokio::time::timeout(timeout, connect).await.ok()?
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

/// Pins the certificate of every profile that needs it. Returns the names of the profiles
/// whose certificate could not be read (they will fail certificate validation in Xray).
pub async fn pin_insecure(profiles: &mut [Profile], global_insecure: bool, timeout: Duration) -> Vec<String> {
    let targets: Vec<(usize, String, u16, String, Vec<String>)> = profiles
        .iter()
        .enumerate()
        .filter(|(_, p)| needs_pin(p, global_insecure))
        .map(|(i, p)| (i, p.address.clone(), p.port, sni_of(p), p.tls.alpn.clone()))
        .collect();
    let results: Vec<(usize, Option<String>)> = stream::iter(targets)
        .map(|(i, host, port, sni, alpn)| async move { (i, leaf_sha256(&host, port, &sni, &alpn, timeout).await) })
        .buffer_unordered(16)
        .collect()
        .await;
    let mut failed = Vec::new();
    for (i, hash) in results {
        match hash {
            Some(hash) => profiles[i].tls.pinned_sha256 = hash,
            None => failed.push(profiles[i].display_name()),
        }
    }
    failed
}
