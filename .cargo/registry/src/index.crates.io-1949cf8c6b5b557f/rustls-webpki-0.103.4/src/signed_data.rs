// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use crate::der::{self, FromDer};
use crate::error::{
    DerTypeId, Error, UnsupportedSignatureAlgorithmContext,
    UnsupportedSignatureAlgorithmForPublicKeyContext,
};
use crate::verify_cert::Budget;

use pki_types::SignatureVerificationAlgorithm;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// X.509 certificates and related items that are signed are almost always
/// encoded in the format "tbs||signatureAlgorithm||signature". This structure
/// captures this pattern as an owned data type.
#[cfg(feature = "alloc")]
#[derive(Clone, Debug)]
pub(crate) struct OwnedSignedData {
    /// The signed data. This would be `tbsCertificate` in the case of an X.509
    /// certificate, `tbsResponseData` in the case of an OCSP response, `tbsCertList`
    /// in the case of a CRL, and the data nested in the `digitally-signed` construct for
    /// TLS 1.2 signed data.
    pub(crate) data: Vec<u8>,

    /// The value of the `AlgorithmIdentifier`. This would be
    /// `signatureAlgorithm` in the case of an X.509 certificate, OCSP
    /// response or CRL. This would have to be synthesized in the case of TLS 1.2
    /// signed data, since TLS does not identify algorithms by ASN.1 OIDs.
    pub(crate) algorithm: Vec<u8>,

    /// The value of the signature. This would be `signature` in an X.509
    /// certificate, OCSP response or CRL. This would be the value of
    /// `DigitallySigned.signature` for TLS 1.2 signed data.
    pub(crate) signature: Vec<u8>,
}

#[cfg(feature = "alloc")]
impl OwnedSignedData {
    /// Return a borrowed [`SignedData`] from the owned representation.
    pub(crate) fn borrow(&self) -> SignedData<'_> {
        SignedData {
            data: untrusted::Input::from(&self.data),
            algorithm: untrusted::Input::from(&self.algorithm),
            signature: untrusted::Input::from(&self.signature),
        }
    }
}

/// X.509 certificates and related items that are signed are almost always
/// encoded in the format "tbs||signatureAlgorithm||signature". This structure
/// captures this pattern.
#[derive(Debug)]
pub(crate) struct SignedData<'a> {
    /// The signed data. This would be `tbsCertificate` in the case of an X.509
    /// certificate, `tbsResponseData` in the case of an OCSP response, `tbsCertList`
    /// in the case of a CRL, and the data nested in the `digitally-signed` construct for
    /// TLS 1.2 signed data.
    pub(crate) data: untrusted::Input<'a>,

    /// The value of the `AlgorithmIdentifier`. This would be
    /// `signatureAlgorithm` in the case of an X.509 certificate, OCSP
    /// response or CRL. This would have to be synthesized in the case of TLS 1.2
    /// signed data, since TLS does not identify algorithms by ASN.1 OIDs.
    pub(crate) algorithm: untrusted::Input<'a>,

    /// The value of the signature. This would be `signature` in an X.509
    /// certificate, OCSP response or CRL. This would be the value of
    /// `DigitallySigned.signature` for TLS 1.2 signed data.
    pub(crate) signature: untrusted::Input<'a>,
}

impl<'a> SignedData<'a> {
    /// Parses the concatenation of "tbs||signatureAlgorithm||signature" that
    /// is common in the X.509 certificate and OCSP response syntaxes.
    ///
    /// X.509 Certificates (RFC 5280) look like this:
    ///
    /// ```ASN.1
    /// Certificate (SEQUENCE) {
    ///     tbsCertificate TBSCertificate,
    ///     signatureAlgorithm AlgorithmIdentifier,
    ///     signatureValue BIT STRING
    /// }
    /// ```
    ///
    /// OCSP responses (RFC 6960) look like this:
    /// ```ASN.1
    /// BasicOCSPResponse {
    ///     tbsResponseData ResponseData,
    ///     signatureAlgorithm AlgorithmIdentifier,
    ///     signature BIT STRING,
    ///     certs [0] EXPLICIT SEQUENCE OF Certificate OPTIONAL
    /// }
    /// ```
    ///
    /// Note that this function does NOT parse the outermost `SEQUENCE` or the
    /// `certs` value.
    ///
    /// The return value's first component is the contents of
    /// `tbsCertificate`/`tbsResponseData`; the second component is a `SignedData`
    /// structure that can be passed to `verify_signed_data`.
    ///
    /// The provided size_limit will enforce the largest possible outermost `SEQUENCE` this
    /// function will read.
    pub(crate) fn from_der(
        der: &mut untrusted::Reader<'a>,
        size_limit: usize,
    ) -> Result<(untrusted::Input<'a>, Self), Error> {
        let (data, tbs) = der.read_partial(|input| {
            der::expect_tag_and_get_value_limited(input, der::Tag::Sequence, size_limit)
        })?;
        let algorithm = der::expect_tag(der, der::Tag::Sequence)?;
        let signature = der::bit_string_with_no_unused_bits(der)?;

        Ok((
            tbs,
            SignedData {
                data,
                algorithm,
                signature,
            },
        ))
    }

    /// Convert the borrowed signed data to an [`OwnedSignedData`].
    #[cfg(feature = "alloc")]
    pub(crate) fn to_owned(&self) -> OwnedSignedData {
        OwnedSignedData {
            data: self.data.as_slice_less_safe().to_vec(),
            algorithm: self.algorithm.as_slice_less_safe().to_vec(),
            signature: self.signature.as_slice_less_safe().to_vec(),
        }
    }
}

/// Verify `signed_data` using the public key in the DER-encoded
/// SubjectPublicKeyInfo `spki` using one of the algorithms in
/// `supported_algorithms`.
///
/// The algorithm is chosen based on the algorithm information encoded in the
/// algorithm identifiers in `public_key` and `signed_data.algorithm`. The
/// ordering of the algorithms in `supported_algorithms` does not really matter,
/// but generally more common algorithms should go first, as it is scanned
/// linearly for matches.
pub(crate) fn verify_signed_data(
    supported_algorithms: &[&dyn SignatureVerificationAlgorithm],
    spki_value: untrusted::Input<'_>,
    signed_data: &SignedData<'_>,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.consume_signature()?;

    // We need to verify the signature in `signed_data` using the public key
    // in `public_key`. In order to know which *ring* signature verification
    // algorithm to use, we need to know the public key algorithm (ECDSA,
    // RSA PKCS#1, etc.), the curve (if applicable), and the digest algorithm.
    // `signed_data` identifies only the public key algorithm and the digest
    // algorithm, and `public_key` identifies only the public key algorithm and
    // the curve (if any). Thus, we have to combine information from both
    // inputs to figure out which `ring::signature::VerificationAlgorithm` to
    // use to verify the signature.
    //
    // This is all further complicated by the fact that we don't have any
    // implicit knowledge about any algorithms or identifiers, since all of
    // that information is encoded in `supported_algorithms.` In particular, we
    // avoid hard-coding any of that information so that (link-time) dead code
    // elimination will work effectively in eliminating code for unused
    // algorithms.

    // Parse the signature.
    //
    let mut invalid_for_public_key = None;
    for supported_alg in supported_algorithms
        .iter()
        .filter(|alg| alg.signature_alg_id().as_ref() == signed_data.algorithm.as_slice_less_safe())
    {
        match verify_signature(
            *supported_alg,
            spki_value,
            signed_data.data,
            signed_data.signature,
        ) {
            Err(Error::UnsupportedSignatureAlgorithmForPublicKeyContext(cx)) => {
                invalid_for_public_key = Some(cx);
                continue;
            }
            result => return result,
        }
    }

    if let Some(cx) = invalid_for_public_key {
        return Err(Error::UnsupportedSignatureAlgorithmForPublicKeyContext(cx));
    }

    Err(Error::UnsupportedSignatureAlgorithmContext(
        UnsupportedSignatureAlgorithmContext {
            #[cfg(feature = "alloc")]
            signature_algorithm_id: signed_data.algorithm.as_slice_less_safe().to_vec(),
            #[cfg(feature = "alloc")]
            supported_algorithms: supported_algorithms
                .iter()
                .map(|&alg| alg.signature_alg_id())
                .collect(),
        },
    ))
}

pub(crate) fn verify_signature(
    signature_alg: &dyn SignatureVerificationAlgorithm,
    spki_value: untrusted::Input<'_>,
    msg: untrusted::Input<'_>,
    signature: untrusted::Input<'_>,
) -> Result<(), Error> {
    let spki = der::read_all::<SubjectPublicKeyInfo<'_>>(spki_value)?;
    if signature_alg.public_key_alg_id().as_ref() != spki.algorithm_id_value.as_slice_less_safe() {
        return Err(Error::UnsupportedSignatureAlgorithmForPublicKeyContext(
            UnsupportedSignatureAlgorithmForPublicKeyContext {
                #[cfg(feature = "alloc")]
                signature_algorithm_id: signature_alg.signature_alg_id().as_ref().to_vec(),
                #[cfg(feature = "alloc")]
                public_key_algorithm_id: spki.algorithm_id_value.as_slice_less_safe().to_vec(),
            },
        ));
    }

    signature_alg
        .verify_signature(
            spki.key_value.as_slice_less_safe(),
            msg.as_slice_less_safe(),
            signature.as_slice_less_safe(),
        )
        .map_err(|_| Error::InvalidSignatureForPublicKey)
}

pub(crate) struct SubjectPublicKeyInfo<'a> {
    algorithm_id_value: untrusted::Input<'a>,
    key_value: untrusted::Input<'a>,
}

impl<'a> FromDer<'a> for SubjectPublicKeyInfo<'a> {
    // Parse the public key into an algorithm OID, an optional curve OID, and the
    // key value. The caller needs to check whether these match the
    // `PublicKeyAlgorithm` for the `SignatureVerificationAlgorithm` that is matched when
    // parsing the signature.
    fn from_der(reader: &mut untrusted::Reader<'a>) -> Result<Self, Error> {
        let algorithm_id_value = der::expect_tag(reader, der::Tag::Sequence)?;
        let key_value = der::bit_string_with_no_unused_bits(reader)?;
        Ok(SubjectPublicKeyInfo {
            algorithm_id_value,
            key_value,
        })
    }

    const TYPE_ID: DerTypeId = DerTypeId::SubjectPublicKeyInfo;
}
