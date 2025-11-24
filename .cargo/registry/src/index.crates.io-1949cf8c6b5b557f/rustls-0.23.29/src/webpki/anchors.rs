use alloc::vec::Vec;
use alloc::{fmt, format};

use pki_types::{CertificateDer, TrustAnchor};
use webpki::anchor_from_trusted_cert;

use super::pki_error;
use crate::log::{debug, trace};
use crate::{DistinguishedName, Error};

/// A container for root certificates able to provide a root-of-trust
/// for connection authentication.
#[derive(Clone)]
pub struct RootCertStore {
    /// The list of roots.
    pub roots: Vec<TrustAnchor<'static>>,
}

impl RootCertStore {
    /// Make a new, empty `RootCertStore`.
    pub fn empty() -> Self {
        Self { roots: Vec::new() }
    }

    /// Parse the given DER-encoded certificates and add all that can be parsed
    /// in a best-effort fashion.
    ///
    /// This is because large collections of root certificates often
    /// include ancient or syntactically invalid certificates.
    ///
    /// Returns the number of certificates added, and the number that were ignored.
    pub fn add_parsable_certificates<'a>(
        &mut self,
        der_certs: impl IntoIterator<Item = CertificateDer<'a>>,
    ) -> (usize, usize) {
        let mut valid_count = 0;
        let mut invalid_count = 0;

        for der_cert in der_certs {
            #[cfg_attr(not(feature = "logging"), allow(unused_variables))]
            match anchor_from_trusted_cert(&der_cert) {
                Ok(anchor) => {
                    self.roots.push(anchor.to_owned());
                    valid_count += 1;
                }
                Err(err) => {
                    trace!("invalid cert der {:?}", der_cert.as_ref());
                    debug!("certificate parsing failed: {err:?}");
                    invalid_count += 1;
                }
            };
        }

        debug!(
            "add_parsable_certificates processed {valid_count} valid and {invalid_count} invalid certs"
        );

        (valid_count, invalid_count)
    }

    /// Add a single DER-encoded certificate to the store.
    ///
    /// This is suitable for a small set of root certificates that are expected to parse
    /// successfully. For large collections of roots (for example from a system store) it
    /// is expected that some of them might not be valid according to the rules rustls
    /// implements. As long as a relatively limited number of certificates are affected,
    /// this should not be a cause for concern. Use [`RootCertStore::add_parsable_certificates`]
    /// in order to add as many valid roots as possible and to understand how many certificates
    /// have been diagnosed as malformed.
    pub fn add(&mut self, der: CertificateDer<'_>) -> Result<(), Error> {
        self.roots.push(
            anchor_from_trusted_cert(&der)
                .map_err(pki_error)?
                .to_owned(),
        );
        Ok(())
    }

    /// Return the DER encoded [`DistinguishedName`] of each trust anchor subject in the root
    /// cert store.
    ///
    /// Each [`DistinguishedName`] will be a DER-encoded X.500 distinguished name, per
    /// [RFC 5280 A.1], including the outer `SEQUENCE`.
    ///
    /// [RFC 5280 A.1]: https://www.rfc-editor.org/rfc/rfc5280#appendix-A.1
    pub fn subjects(&self) -> Vec<DistinguishedName> {
        self.roots
            .iter()
            .map(|ta| DistinguishedName::in_sequence(ta.subject.as_ref()))
            .collect()
    }

    /// Return true if there are no certificates.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Say how many certificates are in the container.
    pub fn len(&self) -> usize {
        self.roots.len()
    }
}

impl FromIterator<TrustAnchor<'static>> for RootCertStore {
    fn from_iter<T: IntoIterator<Item = TrustAnchor<'static>>>(iter: T) -> Self {
        Self {
            roots: iter.into_iter().collect(),
        }
    }
}

impl Extend<TrustAnchor<'static>> for RootCertStore {
    fn extend<T: IntoIterator<Item = TrustAnchor<'static>>>(&mut self, iter: T) {
        self.roots.extend(iter);
    }
}

impl fmt::Debug for RootCertStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootCertStore")
            .field("roots", &format!("({} roots)", &self.roots.len()))
            .finish()
    }
}

#[test]
fn root_cert_store_debug() {
    use core::iter;

    use pki_types::Der;

    let ta = TrustAnchor {
        subject: Der::from_slice(&[]),
        subject_public_key_info: Der::from_slice(&[]),
        name_constraints: None,
    };
    let store = RootCertStore::from_iter(iter::repeat(ta).take(138));

    assert_eq!(
        format!("{store:?}"),
        "RootCertStore { roots: \"(138 roots)\" }"
    );
}
