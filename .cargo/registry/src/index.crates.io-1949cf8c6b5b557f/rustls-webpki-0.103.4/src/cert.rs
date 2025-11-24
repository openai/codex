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

#[cfg(feature = "alloc")]
use pki_types::SubjectPublicKeyInfoDer;
use pki_types::{CertificateDer, DnsName};

use crate::der::{self, CONSTRUCTED, CONTEXT_SPECIFIC, DerIterator, FromDer, Tag};
use crate::error::{DerTypeId, Error};
use crate::public_values_eq;
use crate::signed_data::SignedData;
use crate::subject_name::{GeneralName, NameIterator, WildcardDnsNameRef};
use crate::x509::{DistributionPointName, Extension, remember_extension, set_extension_once};

/// A parsed X509 certificate.
pub struct Cert<'a> {
    pub(crate) serial: untrusted::Input<'a>,
    pub(crate) signed_data: SignedData<'a>,
    pub(crate) issuer: untrusted::Input<'a>,
    pub(crate) validity: untrusted::Input<'a>,
    pub(crate) subject: untrusted::Input<'a>,
    pub(crate) spki: untrusted::Input<'a>,

    pub(crate) basic_constraints: Option<untrusted::Input<'a>>,
    // key usage (KU) extension (if any). When validating certificate revocation lists (CRLs) this
    // field will be consulted to determine if the cert is allowed to sign CRLs. For cert validation
    // this field is ignored (for more detail see in `verify_cert.rs` and
    // `check_issuer_independent_properties`).
    pub(crate) key_usage: Option<untrusted::Input<'a>>,
    pub(crate) eku: Option<untrusted::Input<'a>>,
    pub(crate) name_constraints: Option<untrusted::Input<'a>>,
    pub(crate) subject_alt_name: Option<untrusted::Input<'a>>,
    pub(crate) crl_distribution_points: Option<untrusted::Input<'a>>,

    der: CertificateDer<'a>,
}

impl<'a> Cert<'a> {
    pub(crate) fn from_der(cert_der: untrusted::Input<'a>) -> Result<Self, Error> {
        let (tbs, signed_data) =
            cert_der.read_all(Error::TrailingData(DerTypeId::Certificate), |cert_der| {
                der::nested(
                    cert_der,
                    der::Tag::Sequence,
                    Error::TrailingData(DerTypeId::SignedData),
                    |der| {
                        // limited to SEQUENCEs of size 2^16 or less.
                        SignedData::from_der(der, der::TWO_BYTE_DER_SIZE)
                    },
                )
            })?;

        tbs.read_all(
            Error::TrailingData(DerTypeId::CertificateTbsCertificate),
            |tbs| {
                version3(tbs)?;

                let serial = lenient_certificate_serial_number(tbs)?;

                let signature = der::expect_tag(tbs, der::Tag::Sequence)?;
                // TODO: In mozilla::pkix, the comparison is done based on the
                // normalized value (ignoring whether or not there is an optional NULL
                // parameter for RSA-based algorithms), so this may be too strict.
                if !public_values_eq(signature, signed_data.algorithm) {
                    return Err(Error::SignatureAlgorithmMismatch);
                }

                let issuer = der::expect_tag(tbs, der::Tag::Sequence)?;
                let validity = der::expect_tag(tbs, der::Tag::Sequence)?;
                let subject = der::expect_tag(tbs, der::Tag::Sequence)?;
                let spki = der::expect_tag(tbs, der::Tag::Sequence)?;

                // In theory there could be fields [1] issuerUniqueID and [2]
                // subjectUniqueID, but in practice there never are, and to keep the
                // code small and simple we don't accept any certificates that do
                // contain them.

                let mut cert = Cert {
                    signed_data,
                    serial,
                    issuer,
                    validity,
                    subject,
                    spki,

                    basic_constraints: None,
                    key_usage: None,
                    eku: None,
                    name_constraints: None,
                    subject_alt_name: None,
                    crl_distribution_points: None,

                    der: CertificateDer::from(cert_der.as_slice_less_safe()),
                };

                // When used to read X509v3 Certificate.tbsCertificate.extensions, we allow
                // the extensions to be empty.  This is in spite of RFC5280:
                //
                //   "If present, this field is a SEQUENCE of one or more certificate extensions."
                //
                // Unfortunately other implementations don't get this right, eg:
                // - https://github.com/golang/go/issues/52319
                // - https://github.com/openssl/openssl/issues/20877
                const ALLOW_EMPTY: bool = true;

                if !tbs.at_end() {
                    der::nested(
                        tbs,
                        der::Tag::ContextSpecificConstructed3,
                        Error::TrailingData(DerTypeId::CertificateExtensions),
                        |tagged| {
                            der::nested_of_mut(
                                tagged,
                                der::Tag::Sequence,
                                der::Tag::Sequence,
                                Error::TrailingData(DerTypeId::Extension),
                                ALLOW_EMPTY,
                                |extension| {
                                    remember_cert_extension(
                                        &mut cert,
                                        &Extension::from_der(extension)?,
                                    )
                                },
                            )
                        },
                    )?;
                }

                Ok(cert)
            },
        )
    }

    /// Returns a list of valid DNS names provided in the subject alternative names extension
    ///
    /// This function must not be used to implement custom DNS name verification.
    /// Checking that a certificate is valid for a given subject name should always be done with
    /// [EndEntityCert::verify_is_valid_for_subject_name].
    ///
    /// [EndEntityCert::verify_is_valid_for_subject_name]: crate::EndEntityCert::verify_is_valid_for_subject_name
    pub fn valid_dns_names(&self) -> impl Iterator<Item = &str> {
        NameIterator::new(self.subject_alt_name).filter_map(|result| {
            let presented_id = match result.ok()? {
                GeneralName::DnsName(presented) => presented,
                _ => return None,
            };

            // if the name could be converted to a DNS name, return it; otherwise,
            // keep going.
            let dns_str = core::str::from_utf8(presented_id.as_slice_less_safe()).ok()?;
            match DnsName::try_from(dns_str) {
                Ok(_) => Some(dns_str),
                Err(_) => {
                    match WildcardDnsNameRef::try_from_ascii(presented_id.as_slice_less_safe()) {
                        Ok(wildcard_dns_name) => Some(wildcard_dns_name.as_str()),
                        Err(_) => None,
                    }
                }
            }
        })
    }

    /// Raw DER encoded certificate serial number.
    pub fn serial(&self) -> &[u8] {
        self.serial.as_slice_less_safe()
    }

    /// Raw DER encoded certificate issuer.
    pub fn issuer(&self) -> &[u8] {
        self.issuer.as_slice_less_safe()
    }

    /// Raw DER encoded certificate subject.
    pub fn subject(&self) -> &[u8] {
        self.subject.as_slice_less_safe()
    }

    /// Get the RFC 5280-compliant [`SubjectPublicKeyInfoDer`] (SPKI) of this [`Cert`].
    #[cfg(feature = "alloc")]
    pub fn subject_public_key_info(&self) -> SubjectPublicKeyInfoDer<'static> {
        // Our SPKI representation contains only the content of the RFC 5280 SEQUENCE
        // So we wrap the SPKI contents back into a properly-encoded ASN.1 SEQUENCE
        SubjectPublicKeyInfoDer::from(der::asn1_wrap(
            Tag::Sequence,
            self.spki.as_slice_less_safe(),
        ))
    }

    /// Returns an iterator over the certificate's cRLDistributionPoints extension values, if any.
    pub(crate) fn crl_distribution_points(
        &self,
    ) -> Option<impl Iterator<Item = Result<CrlDistributionPoint<'a>, Error>>> {
        self.crl_distribution_points.map(DerIterator::new)
    }

    /// Raw DER encoded representation of the certificate.
    pub fn der(&self) -> CertificateDer<'a> {
        self.der.clone() // This is cheap, just cloning a reference.
    }
}

// mozilla::pkix supports v1, v2, v3, and v4, including both the implicit
// (correct) and explicit (incorrect) encoding of v1. We allow only v3.
fn version3(input: &mut untrusted::Reader<'_>) -> Result<(), Error> {
    der::nested(
        input,
        der::Tag::ContextSpecificConstructed0,
        Error::UnsupportedCertVersion,
        |input| {
            let version = u8::from_der(input)?;
            if version != 2 {
                // v3
                return Err(Error::UnsupportedCertVersion);
            }
            Ok(())
        },
    )
}

pub(crate) fn lenient_certificate_serial_number<'a>(
    input: &mut untrusted::Reader<'a>,
) -> Result<untrusted::Input<'a>, Error> {
    // https://tools.ietf.org/html/rfc5280#section-4.1.2.2:
    // * Conforming CAs MUST NOT use serialNumber values longer than 20 octets."
    // * "The serial number MUST be a positive integer [...]"
    //
    // However, we don't enforce these constraints, as there are widely-deployed trust anchors
    // and many X.509 implementations in common use that violate these constraints. This is called
    // out by the same section of RFC 5280 as cited above:
    //   Note: Non-conforming CAs may issue certificates with serial numbers
    //   that are negative or zero.  Certificate users SHOULD be prepared to
    //   gracefully handle such certificates.
    der::expect_tag(input, Tag::Integer)
}

fn remember_cert_extension<'a>(
    cert: &mut Cert<'a>,
    extension: &Extension<'a>,
) -> Result<(), Error> {
    // We don't do anything with certificate policies so we can safely ignore
    // all policy-related stuff. We assume that the policy-related extensions
    // are not marked critical.

    remember_extension(extension, |id| {
        let out = match id {
            // id-ce-keyUsage 2.5.29.15.
            15 => &mut cert.key_usage,

            // id-ce-subjectAltName 2.5.29.17
            17 => &mut cert.subject_alt_name,

            // id-ce-basicConstraints 2.5.29.19
            19 => &mut cert.basic_constraints,

            // id-ce-nameConstraints 2.5.29.30
            30 => &mut cert.name_constraints,

            // id-ce-cRLDistributionPoints 2.5.29.31
            31 => &mut cert.crl_distribution_points,

            // id-ce-extKeyUsage 2.5.29.37
            37 => &mut cert.eku,

            // Unsupported extension
            _ => return extension.unsupported(),
        };

        set_extension_once(out, || {
            extension.value.read_all(Error::BadDer, |value| match id {
                // Unlike the other extensions we remember KU is a BitString and not a Sequence. We
                // read the raw bytes here and parse at the time of use.
                15 => Ok(value.read_bytes_to_end()),
                // All other remembered certificate extensions are wrapped in a Sequence.
                _ => der::expect_tag(value, Tag::Sequence),
            })
        })
    })
}

/// A certificate revocation list (CRL) distribution point, describing a source of
/// CRL information for a given certificate as described in RFC 5280 section 4.2.3.13[^1].
///
/// [^1]: <https://datatracker.ietf.org/doc/html/rfc5280#section-4.2.1.13>
pub(crate) struct CrlDistributionPoint<'a> {
    /// distributionPoint describes the location of CRL information.
    distribution_point: Option<untrusted::Input<'a>>,

    /// reasons holds a bit flag set of certificate revocation reasons associated with the
    /// CRL distribution point.
    pub(crate) reasons: Option<der::BitStringFlags<'a>>,

    /// when the CRL issuer is not the certificate issuer, crl_issuer identifies the issuer of the
    /// CRL.
    pub(crate) crl_issuer: Option<untrusted::Input<'a>>,
}

impl<'a> CrlDistributionPoint<'a> {
    /// Return the distribution point names (if any).
    pub(crate) fn names(&self) -> Result<Option<DistributionPointName<'a>>, Error> {
        self.distribution_point
            .map(|input| DistributionPointName::from_der(&mut untrusted::Reader::new(input)))
            .transpose()
    }
}

impl<'a> FromDer<'a> for CrlDistributionPoint<'a> {
    fn from_der(reader: &mut untrusted::Reader<'a>) -> Result<Self, Error> {
        // RFC 5280 section §4.2.1.13:
        //   A DistributionPoint consists of three fields, each of which is optional:
        //   distributionPoint, reasons, and cRLIssuer.
        let mut result = CrlDistributionPoint {
            distribution_point: None,
            reasons: None,
            crl_issuer: None,
        };

        der::nested(
            reader,
            Tag::Sequence,
            Error::TrailingData(Self::TYPE_ID),
            |der| {
                const DISTRIBUTION_POINT_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED;
                const REASONS_TAG: u8 = CONTEXT_SPECIFIC | 1;
                const CRL_ISSUER_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED | 2;

                while !der.at_end() {
                    let (tag, value) = der::read_tag_and_get_value(der)?;
                    match tag {
                        DISTRIBUTION_POINT_TAG => {
                            set_extension_once(&mut result.distribution_point, || Ok(value))?
                        }
                        REASONS_TAG => set_extension_once(&mut result.reasons, || {
                            der::bit_string_flags(value)
                        })?,
                        CRL_ISSUER_TAG => set_extension_once(&mut result.crl_issuer, || Ok(value))?,
                        _ => return Err(Error::BadDer),
                    }
                }

                // RFC 5280 section §4.2.1.13:
                //   a DistributionPoint MUST NOT consist of only the reasons field; either distributionPoint or
                //   cRLIssuer MUST be present.
                match (result.distribution_point, result.crl_issuer) {
                    (None, None) => Err(Error::MalformedExtensions),
                    _ => Ok(result),
                }
            },
        )
    }

    const TYPE_ID: DerTypeId = DerTypeId::CrlDistributionPoint;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "alloc")]
    use crate::crl::RevocationReason;
    use std::prelude::v1::*;

    #[test]
    // Note: cert::parse_cert is crate-local visibility, and EndEntityCert doesn't expose the
    //       inner Cert, or the serial number. As a result we test that the raw serial value
    //       is read correctly here instead of in tests/integration.rs.
    fn test_serial_read() {
        let ee = include_bytes!("../tests/misc/serial_neg_ee.der");
        let cert = Cert::from_der(untrusted::Input::from(ee)).expect("failed to parse certificate");
        assert_eq!(cert.serial.as_slice_less_safe(), &[255, 33, 82, 65, 17]);

        let ee = include_bytes!("../tests/misc/serial_large_positive.der");
        let cert = Cert::from_der(untrusted::Input::from(ee)).expect("failed to parse certificate");
        assert_eq!(
            cert.serial.as_slice_less_safe(),
            &[
                0, 230, 9, 254, 122, 234, 0, 104, 140, 224, 36, 180, 237, 32, 27, 31, 239, 82, 180,
                68, 209
            ]
        )
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn test_spki_read() {
        let ee = include_bytes!("../tests/ed25519/ee.der");
        let cert = Cert::from_der(untrusted::Input::from(ee)).expect("failed to parse certificate");
        // How did I get this lovely string of hex bytes?
        // openssl x509 -in tests/ed25519/ee.der -pubkey -noout > pubkey.pem
        // openssl ec -pubin -in pubkey.pem -outform DER -out pubkey.der
        // xxd -plain -cols 1 pubkey.der
        let expected_spki = [
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00, 0xfe, 0x5a,
            0x1e, 0x36, 0x6c, 0x17, 0x27, 0x5b, 0xf1, 0x58, 0x1e, 0x3a, 0x0e, 0xe6, 0x56, 0x29,
            0x8d, 0x9e, 0x1b, 0x3f, 0xd3, 0x3f, 0x96, 0x46, 0xef, 0xbf, 0x04, 0x6b, 0xc7, 0x3d,
            0x47, 0x5c,
        ];
        assert_eq!(expected_spki, *cert.subject_public_key_info())
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_netflix() {
        let ee = include_bytes!("../tests/netflix/ee.der");
        let inter = include_bytes!("../tests/netflix/inter.der");
        let ee_cert = Cert::from_der(untrusted::Input::from(ee)).expect("failed to parse EE cert");
        let cert =
            Cert::from_der(untrusted::Input::from(inter)).expect("failed to parse certificate");

        // The end entity certificate shouldn't have a distribution point.
        assert!(ee_cert.crl_distribution_points.is_none());

        // We expect to be able to parse the intermediate certificate's CRL distribution points.
        let crl_distribution_points = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse distribution points");

        // There should be one distribution point present.
        assert_eq!(crl_distribution_points.len(), 1);
        let crl_distribution_point = crl_distribution_points
            .first()
            .expect("missing distribution point");

        // The distribution point shouldn't have revocation reasons listed.
        assert!(crl_distribution_point.reasons.is_none());

        // The distribution point shouldn't have a CRL issuer listed.
        assert!(crl_distribution_point.crl_issuer.is_none());

        // We should be able to parse the distribution point name.
        let distribution_point_name = crl_distribution_point
            .names()
            .expect("failed to parse distribution point names")
            .expect("missing distribution point name");

        // We expect the distribution point name to be a sequence of GeneralNames, not a name
        // relative to the CRL issuer.
        let names = match distribution_point_name {
            DistributionPointName::NameRelativeToCrlIssuer => {
                panic!("unexpected name relative to crl issuer")
            }
            DistributionPointName::FullName(names) => names,
        };

        // The general names should parse.
        let names = names
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse general names");

        // There should be one general name.
        assert_eq!(names.len(), 1);
        let name = names.first().expect("missing general name");

        // The general name should be a URI matching the expected value.
        match name {
            GeneralName::UniformResourceIdentifier(uri) => {
                assert_eq!(
                    uri.as_slice_less_safe(),
                    "http://s.symcb.com/pca3-g3.crl".as_bytes()
                );
            }
            _ => panic!("unexpected general name type"),
        }
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_with_reasons() {
        let der = include_bytes!("../tests/crl_distrib_point/with_reasons.der");
        let cert =
            Cert::from_der(untrusted::Input::from(der)).expect("failed to parse certificate");

        // We expect to be able to parse the intermediate certificate's CRL distribution points.
        let crl_distribution_points = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse distribution points");

        // There should be one distribution point present.
        assert_eq!(crl_distribution_points.len(), 1);
        let crl_distribution_point = crl_distribution_points
            .first()
            .expect("missing distribution point");

        // The distribution point should include the expected revocation reasons, and no others.
        let reasons = crl_distribution_point
            .reasons
            .as_ref()
            .expect("missing revocation reasons");
        let expected = &[
            RevocationReason::KeyCompromise,
            RevocationReason::AffiliationChanged,
        ];
        for reason in RevocationReason::iter() {
            #[allow(clippy::as_conversions)]
            // revocation reason is u8, infallible to convert to usize.
            match expected.contains(&reason) {
                true => assert!(reasons.bit_set(reason as usize)),
                false => assert!(!reasons.bit_set(reason as usize)),
            }
        }
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_with_crl_issuer() {
        let der = include_bytes!("../tests/crl_distrib_point/with_crl_issuer.der");
        let cert =
            Cert::from_der(untrusted::Input::from(der)).expect("failed to parse certificate");

        // We expect to be able to parse the intermediate certificate's CRL distribution points.
        let crl_distribution_points = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse distribution points");

        // There should be one distribution point present.
        assert_eq!(crl_distribution_points.len(), 1);
        let crl_distribution_point = crl_distribution_points
            .first()
            .expect("missing distribution point");

        // The CRL issuer should be present, but not anything else.
        assert!(crl_distribution_point.crl_issuer.is_some());
        assert!(crl_distribution_point.distribution_point.is_none());
        assert!(crl_distribution_point.reasons.is_none());
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_bad_der() {
        // Created w/
        //   ascii2der -i tests/crl_distrib_point/unknown_tag.der.txt -o tests/crl_distrib_point/unknown_tag.der
        let der = include_bytes!("../tests/crl_distrib_point/unknown_tag.der");
        let cert =
            Cert::from_der(untrusted::Input::from(der)).expect("failed to parse certificate");

        // We expect there to be a distribution point extension, but parsing it should fail
        // due to the unknown tag in the SEQUENCE.
        let result = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>();
        assert!(matches!(result, Err(Error::BadDer)));
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_only_reasons() {
        // Created w/
        //   ascii2der -i tests/crl_distrib_point/only_reasons.der.txt -o tests/crl_distrib_point/only_reasons.der
        let der = include_bytes!("../tests/crl_distrib_point/only_reasons.der");
        let cert =
            Cert::from_der(untrusted::Input::from(der)).expect("failed to parse certificate");

        // We expect there to be a distribution point extension, but parsing it should fail
        // because no distribution points or cRLIssuer are set in the SEQUENCE, just reason codes.
        let result = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>();
        assert!(matches!(result, Err(Error::MalformedExtensions)));
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_name_relative_to_issuer() {
        let der = include_bytes!("../tests/crl_distrib_point/dp_name_relative_to_issuer.der");
        let cert =
            Cert::from_der(untrusted::Input::from(der)).expect("failed to parse certificate");

        // We expect to be able to parse the intermediate certificate's CRL distribution points.
        let crl_distribution_points = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse distribution points");

        // There should be one distribution point present.
        assert_eq!(crl_distribution_points.len(), 1);
        let crl_distribution_point = crl_distribution_points
            .first()
            .expect("missing distribution point");

        assert!(crl_distribution_point.crl_issuer.is_none());
        assert!(crl_distribution_point.reasons.is_none());

        // We should be able to parse the distribution point name.
        let distribution_point_name = crl_distribution_point
            .names()
            .expect("failed to parse distribution point names")
            .expect("missing distribution point name");

        // We expect the distribution point name to be a name relative to the CRL issuer.
        assert!(matches!(
            distribution_point_name,
            DistributionPointName::NameRelativeToCrlIssuer
        ));
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_unknown_name_tag() {
        // Created w/
        //   ascii2der -i tests/crl_distrib_point/unknown_dp_name_tag.der.txt > tests/crl_distrib_point/unknown_dp_name_tag.der
        let der = include_bytes!("../tests/crl_distrib_point/unknown_dp_name_tag.der");
        let cert =
            Cert::from_der(untrusted::Input::from(der)).expect("failed to parse certificate");

        // We expect to be able to parse the intermediate certificate's CRL distribution points.
        let crl_distribution_points = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse distribution points");

        // There should be one distribution point present.
        assert_eq!(crl_distribution_points.len(), 1);
        let crl_distribution_point = crl_distribution_points
            .first()
            .expect("missing distribution point");

        // Parsing the distrubition point names should fail due to the unknown name tag.
        let result = crl_distribution_point.names();
        assert!(matches!(result, Err(Error::BadDer)))
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_crl_distribution_point_multiple() {
        let der = include_bytes!("../tests/crl_distrib_point/multiple_distribution_points.der");
        let cert =
            Cert::from_der(untrusted::Input::from(der)).expect("failed to parse certificate");

        // We expect to be able to parse the intermediate certificate's CRL distribution points.
        let crl_distribution_points = cert
            .crl_distribution_points()
            .expect("missing distribution points extension")
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse distribution points");

        // There should be two distribution points present.
        let (point_a, point_b) = (
            crl_distribution_points
                .first()
                .expect("missing first distribution point"),
            crl_distribution_points
                .get(1)
                .expect("missing second distribution point"),
        );

        fn get_names<'a>(
            point: &'a CrlDistributionPoint<'a>,
        ) -> impl Iterator<Item = Result<GeneralName<'a>, Error>> {
            match point
                .names()
                .expect("failed to parse distribution point names")
                .expect("missing distribution point name")
            {
                DistributionPointName::NameRelativeToCrlIssuer => {
                    panic!("unexpected relative name")
                }
                DistributionPointName::FullName(names) => names,
            }
        }

        fn uri_bytes<'a>(name: &'a GeneralName<'a>) -> &'a [u8] {
            match name {
                GeneralName::UniformResourceIdentifier(uri) => uri.as_slice_less_safe(),
                _ => panic!("unexpected name type"),
            }
        }

        // We expect to find three URIs across the two distribution points.
        let expected_names = [
            "http://example.com/crl.1.der".as_bytes(),
            "http://example.com/crl.2.der".as_bytes(),
            "http://example.com/crl.3.der".as_bytes(),
        ];
        let all_names = get_names(point_a)
            .chain(get_names(point_b))
            .collect::<Result<Vec<_>, Error>>()
            .expect("failed to parse names");

        assert_eq!(
            all_names.iter().map(uri_bytes).collect::<Vec<_>>(),
            expected_names
        );
    }
}
