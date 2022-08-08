use anyhow::Result;

use rcgen::{CertificateParams, DistinguishedName};
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
    scheme: tokio_rustls::rustls::internal::msgs::enums::SignatureScheme,
) -> Result<x509_signature::SignatureScheme, TlsError> {
    use tokio_rustls::rustls::internal::msgs::enums::SignatureScheme as R;
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

pub struct ServerVerifier;

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
        dss: &tokio_rustls::rustls::internal::msgs::handshake::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::HandshakeSignatureValid, tokio_rustls::rustls::Error>
    {
        let cert = get_cert(cert)?;
        let scheme = convert_scheme(dss.scheme)?;
        let signature = dss.sig.0.as_ref();

        cert.check_tls12_signature(scheme, message, signature)
            .map(|_| tokio_rustls::rustls::client::HandshakeSignatureValid::assertion())
            .map_err(|_| TlsError::InvalidCertificateSignature)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &tokio_rustls::rustls::Certificate,
        dss: &tokio_rustls::rustls::internal::msgs::handshake::DigitallySignedStruct,
    ) -> Result<tokio_rustls::rustls::client::HandshakeSignatureValid, tokio_rustls::rustls::Error>
    {
        let cert = get_cert(cert)?;
        let scheme = convert_scheme(dss.scheme)?;
        let signature = dss.sig.0.as_ref();

        cert.check_tls13_signature(scheme, message, signature)
            .map(|_| tokio_rustls::rustls::client::HandshakeSignatureValid::assertion())
            .map_err(|_| TlsError::InvalidCertificateSignature)
    }

    // fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
    //     tokio_rustls::rustls::client::WebPkiVerifier::verification_schemes()
    // }
}

fn generate_certs() -> Result<(Vec<u8>, Vec<u8>)> {
    let mut cert_params = CertificateParams::new(vec![]);

    let mut dn = DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, "LycoReco");
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

fn load_certs() -> Result<(Vec<u8>, Vec<u8>)> {
    let cert_der = std::fs::read("cert.der")?;
    let key_der = std::fs::read("key.der")?;

    Ok((cert_der, key_der))
}

pub fn load_or_generate_certs() -> Result<(Vec<u8>, Vec<u8>)> {
    match load_certs() {
        Ok(certs) => Ok(certs),
        Err(_) => {
            let (cert_der, key_der) = generate_certs()?;

            std::fs::write("cert.der", &cert_der)?;
            std::fs::write("key.der", &key_der)?;

            Ok((cert_der, key_der))
        }
    }
}
