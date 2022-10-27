use anyhow::Result;

use rcgen::{CertificateParams, DistinguishedName};
use tokio_rustls::rustls;
use tokio_rustls::rustls::Error as TlsError;

/// Parse a `rustls::Certificate` as an `x509_signature::X509Certificate`, if possible.
fn get_cert(
    c: &tokio_rustls::rustls::Certificate,
) -> Result<x509_signature::X509Certificate, TlsError> {
    x509_signature::parse_certificate(c.as_ref()).map_err(|e| {
        TlsError::InvalidCertificateData(format!("Failed to parse certificate: {:?}", e))
    })
}

/// Convert from the signature scheme type used in `rustls` to the one used in
/// `x509_signature`.
///
/// (We can't just use the x509_signature crate's "rustls" feature to have it
/// use the same enum from `rustls`, because it seems to be on a different
/// version from the rustls we want.)
fn convert_scheme(
    scheme: tokio_rustls::rustls::SignatureScheme,
) -> Result<x509_signature::SignatureScheme, TlsError> {
    use tokio_rustls::rustls::SignatureScheme as R;
    use x509_signature::SignatureScheme as X;

    Ok(match scheme {
        R::RSA_PKCS1_SHA256 => X::RSA_PKCS1_SHA256,
        R::ECDSA_NISTP256_SHA256 => X::ECDSA_NISTP256_SHA256,
        R::RSA_PKCS1_SHA384 => X::RSA_PKCS1_SHA384,
        R::ECDSA_NISTP384_SHA384 => X::ECDSA_NISTP384_SHA384,
        R::RSA_PKCS1_SHA512 => X::RSA_PKCS1_SHA512,
        R::RSA_PSS_SHA256 => X::RSA_PSS_SHA256,
        R::RSA_PSS_SHA384 => X::RSA_PSS_SHA384,
        R::RSA_PSS_SHA512 => X::RSA_PSS_SHA512,
        R::ED25519 => X::ED25519,
        R::ED448 => X::ED448,
        R::RSA_PKCS1_SHA1 | R::ECDSA_SHA1_Legacy | R::ECDSA_NISTP521_SHA512 => {
            // The `x509-signature` crate doesn't support these, nor should it really.
            return Err(TlsError::PeerIncompatibleError(format!(
                "Unsupported signature scheme {:?}",
                scheme
            )));
        }
        R::Unknown(_) => {
            return Err(TlsError::PeerIncompatibleError(format!(
                "Unrecognized signature scheme {:?}",
                scheme
            )))
        }
    })
}

/// A TLS server verifier that does not actually verify the certificate.
pub enum ServerVerifier {
    /// A server verifier that always returns `Ok`.
    AlwaysOk,
    /// A server verifier that returns `Ok` for a particular certificate.
    Single(rustls::Certificate),
}

// https://github.com/c4dt/arti/commit/8def5a0d89603c8f1cfd91109bb439f1881d968f
impl tokio_rustls::rustls::client::ServerCertVerifier for ServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &tokio_rustls::rustls::Certificate,
        _intermediates: &[tokio_rustls::rustls::Certificate],
        _server_name: &tokio_rustls::rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<tokio_rustls::rustls::client::ServerCertVerified, tokio_rustls::rustls::Error> {
        let _cert = get_cert(end_entity)?;
        Ok(tokio_rustls::rustls::client::ServerCertVerified::assertion())
    }

    fn request_scts(&self) -> bool {
        false
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &tokio_rustls::rustls::Certificate,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::HandshakeSignatureValid, tokio_rustls::rustls::Error>
    {
        let cert = get_cert(cert)?;
        let scheme = convert_scheme(dss.scheme)?;
        let signature = dss.signature();

        cert.check_signature(scheme, message, signature)
            .map(|_| tokio_rustls::rustls::client::HandshakeSignatureValid::assertion())
            .map_err(|_| TlsError::InvalidCertificateSignature)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &tokio_rustls::rustls::Certificate,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::HandshakeSignatureValid, tokio_rustls::rustls::Error>
    {
        let cert = get_cert(cert)?;
        let scheme = convert_scheme(dss.scheme)?;
        let signature = dss.signature();

        cert.check_tls13_signature(scheme, message, signature)
            .map(|_| tokio_rustls::rustls::client::HandshakeSignatureValid::assertion())
            .map_err(|_| TlsError::InvalidCertificateSignature)
    }
}

/// A TLS client verifier that does not actually verify the certificate.
pub enum ClientVerifier {
    /// A client verifier that always returns `Ok`.
    AlwaysOk,
    /// A client verifier that returns `Ok` for a particular certificate.
    Single(rustls::Certificate),
}

impl tokio_rustls::rustls::server::ClientCertVerifier for ClientVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> Option<bool> {
        Some(false)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &tokio_rustls::rustls::Certificate,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::HandshakeSignatureValid, TlsError> {
        let cert = get_cert(cert)?;
        let scheme = convert_scheme(dss.scheme)?;
        let signature = dss.signature();

        cert.check_signature(scheme, message, signature)
            .map(|_| tokio_rustls::rustls::client::HandshakeSignatureValid::assertion())
            .map_err(|_| TlsError::InvalidCertificateSignature)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &tokio_rustls::rustls::Certificate,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::HandshakeSignatureValid, TlsError> {
        let cert = get_cert(cert)?;
        let scheme = convert_scheme(dss.scheme)?;
        let signature = dss.signature();

        cert.check_tls13_signature(scheme, message, signature)
            .map(|_| tokio_rustls::rustls::client::HandshakeSignatureValid::assertion())
            .map_err(|_| TlsError::InvalidCertificateSignature)
    }

    fn client_auth_root_subjects(&self) -> Option<tokio_rustls::rustls::DistinguishedNames> {
        Some(tokio_rustls::rustls::DistinguishedNames::new())
    }

    fn verify_client_cert(
        &self,
        end_entity: &tokio_rustls::rustls::Certificate,
        _intermediates: &[tokio_rustls::rustls::Certificate],
        _now: std::time::SystemTime,
    ) -> Result<tokio_rustls::rustls::server::ClientCertVerified, TlsError> {
        let _cert = get_cert(end_entity)?;
        Ok(tokio_rustls::rustls::server::ClientCertVerified::assertion())
    }
}

pub fn generate_certs(device_id: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut cert_params = CertificateParams::new(vec![]);

    let mut dn = DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, device_id);
    dn.push(rcgen::DnType::OrganizationName, "KDE");
    dn.push(rcgen::DnType::OrganizationalUnitName, "KDE Connect");
    cert_params.distinguished_name = dn;

    let now_utc = time::OffsetDateTime::now_utc();
    cert_params.not_before = now_utc - time::Duration::WEEK * 7;
    cert_params.not_after = now_utc + time::Duration::DAY * 365 * 10;

    let cert = rcgen::Certificate::from_params(cert_params)?;

    let key_der = cert.serialize_private_key_der();
    let cert_der = cert.serialize_der()?;

    Ok((cert_der, key_der))
}
