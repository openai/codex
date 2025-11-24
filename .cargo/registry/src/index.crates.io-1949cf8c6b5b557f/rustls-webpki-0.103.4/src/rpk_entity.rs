use crate::error::Error;
use crate::signed_data::SubjectPublicKeyInfo;
use crate::{DerTypeId, der, signed_data};
use pki_types::{SignatureVerificationAlgorithm, SubjectPublicKeyInfoDer};

/// A Raw Public Key, used for connections using raw public keys as specified
/// in [RFC 7250](https://www.rfc-editor.org/rfc/rfc7250).
#[derive(Debug)]
pub struct RawPublicKeyEntity<'a> {
    inner: untrusted::Input<'a>,
}

impl<'a> TryFrom<&'a SubjectPublicKeyInfoDer<'a>> for RawPublicKeyEntity<'a> {
    type Error = Error;

    /// Parse the ASN.1 DER-encoded SPKI encoding of the raw public key `spki`.
    /// Since we are parsing a raw public key, we first strip the outer sequence tag.
    fn try_from(spki: &'a SubjectPublicKeyInfoDer<'a>) -> Result<Self, Self::Error> {
        let input = untrusted::Input::from(spki.as_ref());
        let spki = input.read_all(
            Error::TrailingData(DerTypeId::SubjectPublicKeyInfo),
            |reader| {
                let untagged_spki = der::expect_tag(reader, der::Tag::Sequence)?;
                der::read_all::<SubjectPublicKeyInfo<'_>>(untagged_spki)?;
                Ok(untagged_spki)
            },
        )?;
        Ok(Self { inner: spki })
    }
}

impl RawPublicKeyEntity<'_> {
    /// Verifies the signature `signature` of message `msg` using a raw public key,
    /// supporting RFC 7250.
    ///
    /// For more information on `signature_alg` and `signature` see the documentation for [`crate::end_entity::EndEntityCert::verify_signature`].
    pub fn verify_signature(
        &self,
        signature_alg: &dyn SignatureVerificationAlgorithm,
        msg: &[u8],
        signature: &[u8],
    ) -> Result<(), Error> {
        signed_data::verify_signature(
            signature_alg,
            self.inner,
            untrusted::Input::from(msg),
            untrusted::Input::from(signature),
        )
    }
}

#[cfg(feature = "alloc")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ee_read_for_rpk() {
        // Try to read an end entity certificate into a RawPublicKeyEntity.
        // It will fail to parse the key value since we expect no unused bits.
        let ee = include_bytes!("../tests/ed25519/ee.der");
        let ee_der = SubjectPublicKeyInfoDer::from(ee.as_slice());
        assert_eq!(
            RawPublicKeyEntity::try_from(&ee_der).expect_err("unexpectedly parsed certificate"),
            Error::TrailingData(DerTypeId::BitString)
        );
    }

    #[test]
    fn test_spki_read_for_rpk() {
        let pubkey = include_bytes!("../tests/ed25519/ee-pubkey.der");
        let spki_der = SubjectPublicKeyInfoDer::from(pubkey.as_slice());
        let rpk = RawPublicKeyEntity::try_from(&spki_der).expect("failed to parse rpk");

        // Retrieved the SPKI from the pubkey.der using the following commands (as in [`cert::test_spki_read`]):
        // xxd -plain -cols 1 tests/ed255519/ee-pubkey.der
        let expected_spki = [
            0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00, 0xfe, 0x5a, 0x1e, 0x36,
            0x6c, 0x17, 0x27, 0x5b, 0xf1, 0x58, 0x1e, 0x3a, 0x0e, 0xe6, 0x56, 0x29, 0x8d, 0x9e,
            0x1b, 0x3f, 0xd3, 0x3f, 0x96, 0x46, 0xef, 0xbf, 0x04, 0x6b, 0xc7, 0x3d, 0x47, 0x5c,
        ];
        assert_eq!(expected_spki, rpk.inner.as_slice_less_safe())
    }
}
