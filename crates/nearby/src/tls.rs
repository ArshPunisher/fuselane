//! TLS for Nearby. Certificates are self-signed (as in LocalSend), so trust
//! isn't from a certificate authority: a sending Fuselane pins the receiver's
//! certificate to the fingerprint it announced, and refuses any other.

use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, ring};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

use crate::identity::{Identity, fingerprint_of};

fn provider() -> Arc<CryptoProvider> {
    Arc::new(ring::default_provider())
}

/// The server side: presents this device's certificate.
pub fn server_config(id: &Identity) -> Result<Arc<rustls::ServerConfig>, rustls::Error> {
    let mut c = rustls::ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(vec![id.cert.clone()], id.private_key())?;
    c.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(c))
}

/// Accepts exactly one certificate fingerprint (or any, when `expected` is
/// None, for LocalSend devices that announced no usable one), and records the
/// fingerprint it saw. Signatures are still checked, so the peer must hold the
/// certificate's key.
#[derive(Debug)]
pub struct Pinned {
    expected: Option<String>,
    pub seen: Mutex<Option<String>>,
    algs: rustls::crypto::WebPkiSupportedAlgorithms,
}

impl Pinned {
    pub fn new(expected: Option<&str>) -> Arc<Pinned> {
        Arc::new(Pinned {
            expected: expected.map(str::to_ascii_uppercase),
            seen: Mutex::new(None),
            algs: ring::default_provider().signature_verification_algorithms,
        })
    }
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint_of(end_entity);
        *self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(fp.clone());
        match &self.expected {
            Some(want) if *want != fp => Err(rustls::Error::General(
                "this device's certificate isn't the one it announced".into(),
            )),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// The client side, pinned to `verifier`.
pub fn client_config(verifier: Arc<Pinned>) -> Result<Arc<rustls::ClientConfig>, rustls::Error> {
    let mut c = rustls::ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth();
    c.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(c))
}
