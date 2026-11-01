//! TLS handshake for metadata only. Certificates are *not* validated
//! (devices on a LAN use self-signed certificates); nothing is uploaded.

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::{Arc, OnceLock};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertInfo {
    pub subject_cn: Option<String>,
    pub san: Vec<String>,
    pub issuer: Option<String>,
    pub not_before: Option<String>,
    pub not_after: Option<String>,
    pub self_signed: bool,
}

#[derive(Debug)]
struct AcceptAny(Arc<CryptoProvider>);

impl ServerCertVerifier for AcceptAny {
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

fn connector() -> Option<TlsConnector> {
    static CFG: OnceLock<Option<Arc<ClientConfig>>> = OnceLock::new();
    CFG.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = ClientConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .ok()?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAny(provider)))
            .with_no_client_auth();
        Some(Arc::new(cfg))
    })
    .clone()
    .map(TlsConnector::from)
}

/// Perform a TLS handshake over an established TCP stream.
pub async fn handshake(tcp: TcpStream, ip: IpAddr) -> Option<(TlsStream<TcpStream>, Option<CertInfo>)> {
    let conn = connector()?;
    let name = ServerName::IpAddress(ip.into());
    let tls = conn.connect(name, tcp).await.ok()?;
    let info = tls.get_ref().1.peer_certificates().and_then(|c| c.first()).and_then(|der| parse_certificate(der.as_ref()));
    Some((tls, info))
}

pub fn parse_certificate(der: &[u8]) -> Option<CertInfo> {
    let (_, cert) = x509_parser::parse_x509_certificate(der).ok()?;
    let cn = |name: &x509_parser::x509::X509Name<'_>| name.iter_common_name().next().and_then(|a| a.as_str().ok()).map(str::to_string);
    let subject_cn = cn(cert.subject());
    let issuer = cn(cert.issuer()).or_else(|| Some(cert.issuer().to_string()));
    let mut san = Vec::new();
    if let Ok(Some(ext)) = cert.subject_alternative_name() {
        for gn in &ext.value.general_names {
            match gn {
                x509_parser::extensions::GeneralName::DNSName(d) => san.push(d.to_string()),
                x509_parser::extensions::GeneralName::IPAddress(b) if b.len() == 4 => {
                    san.push(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]));
                }
                _ => {}
            }
        }
    }
    let validity = cert.validity();
    Some(CertInfo {
        subject_cn,
        san,
        issuer,
        not_before: Some(validity.not_before.to_string()),
        not_after: Some(validity.not_after.to_string()),
        self_signed: cert.subject() == cert.issuer(),
    })
}
