//! This device's key and self-signed certificate. Made once and kept in the
//! app's data folder; the certificate's SHA-256 is the device's fingerprint,
//! the thing other devices trust.

use std::path::Path;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sha2::Digest;

/// This device's identity.
#[derive(Debug)]
pub struct Identity {
    pub cert: CertificateDer<'static>,
    key: Vec<u8>,
    pub fingerprint: String,
}

impl Clone for Identity {
    fn clone(&self) -> Self {
        Identity {
            cert: self.cert.clone(),
            key: self.key.clone(),
            fingerprint: self.fingerprint.clone(),
        }
    }
}

/// SHA-256 of a certificate, as uppercase hex (how LocalSend writes it).
pub fn fingerprint_of(cert: &[u8]) -> String {
    sha2::Sha256::digest(cert)
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

impl Identity {
    /// Loads the identity from `dir`, or makes and saves a new one.
    pub fn load_or_create(dir: &Path) -> std::io::Result<Identity> {
        let (cert_path, key_path) = (dir.join("nearby-cert.der"), dir.join("nearby-key.der"));
        if let (Ok(cert), Ok(key)) = (std::fs::read(&cert_path), std::fs::read(&key_path))
            && !cert.is_empty()
            && !key.is_empty()
        {
            return Ok(Identity::from_parts(cert, key));
        }
        let made = rcgen::generate_simple_self_signed(vec!["fuselane.local".to_string()])
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let cert = made.cert.der().to_vec();
        let key = made.key_pair.serialize_der();
        std::fs::create_dir_all(dir)?;
        write_private(&key_path, &key)?;
        std::fs::write(&cert_path, &cert)?;
        Ok(Identity::from_parts(cert, key))
    }

    fn from_parts(cert: Vec<u8>, key: Vec<u8>) -> Identity {
        Identity {
            fingerprint: fingerprint_of(&cert),
            cert: CertificateDer::from(cert),
            key,
        }
    }

    pub fn private_key(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.key.clone()))
    }
}

/// Writes the key readable by this user only (on Unix; Windows profiles are per user).
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        f.write_all(bytes)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_is_made_once_and_kept() {
        let d = tempfile::tempdir().unwrap();
        let a = Identity::load_or_create(d.path()).unwrap();
        let b = Identity::load_or_create(d.path()).unwrap();
        assert_eq!(a.fingerprint, b.fingerprint);
        assert_eq!(a.fingerprint.len(), 64);
        assert_eq!(a.fingerprint, fingerprint_of(&a.cert));
        let other = tempfile::tempdir().unwrap();
        assert_ne!(
            Identity::load_or_create(other.path()).unwrap().fingerprint,
            a.fingerprint
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(d.path().join("nearby-key.der"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "the key is private");
        }
    }
}
