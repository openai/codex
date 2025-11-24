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

use crate::der::{self, CONSTRUCTED, CONTEXT_SPECIFIC, DerIterator, FromDer};
use crate::error::{DerTypeId, Error};
use crate::subject_name::GeneralName;

pub(crate) struct Extension<'a> {
    pub(crate) critical: bool,
    pub(crate) id: untrusted::Input<'a>,
    pub(crate) value: untrusted::Input<'a>,
}

impl Extension<'_> {
    pub(crate) fn unsupported(&self) -> Result<(), Error> {
        match self.critical {
            true => Err(Error::UnsupportedCriticalExtension),
            false => Ok(()),
        }
    }
}

impl<'a> FromDer<'a> for Extension<'a> {
    fn from_der(reader: &mut untrusted::Reader<'a>) -> Result<Self, Error> {
        let id = der::expect_tag(reader, der::Tag::OID)?;
        let critical = bool::from_der(reader)?;
        let value = der::expect_tag(reader, der::Tag::OctetString)?;
        Ok(Extension {
            id,
            critical,
            value,
        })
    }

    const TYPE_ID: DerTypeId = DerTypeId::Extension;
}

pub(crate) fn set_extension_once<T>(
    destination: &mut Option<T>,
    parser: impl Fn() -> Result<T, Error>,
) -> Result<(), Error> {
    match destination {
        // The extension value has already been set, indicating that we encountered it
        // more than once in our serialized data. That's invalid!
        Some(..) => Err(Error::ExtensionValueInvalid),
        None => {
            *destination = Some(parser()?);
            Ok(())
        }
    }
}

pub(crate) fn remember_extension(
    extension: &Extension<'_>,
    mut handler: impl FnMut(u8) -> Result<(), Error>,
) -> Result<(), Error> {
    // ISO arc for standard certificate and CRL extensions.
    // https://www.rfc-editor.org/rfc/rfc5280#appendix-A.2
    static ID_CE: [u8; 2] = oid![2, 5, 29];

    if extension.id.len() != ID_CE.len() + 1
        || !extension.id.as_slice_less_safe().starts_with(&ID_CE)
    {
        return extension.unsupported();
    }

    // safety: we verify len is non-zero and has the correct prefix above.
    let last_octet = *extension.id.as_slice_less_safe().last().unwrap();
    handler(last_octet)
}

/// A certificate revocation list (CRL) distribution point name, describing a source of
/// CRL information for a given certificate as described in RFC 5280 section 4.2.3.13[^1].
///
/// [^1]: <https://datatracker.ietf.org/doc/html/rfc5280#section-4.2.1.13>
pub(crate) enum DistributionPointName<'a> {
    /// The distribution point name is a relative distinguished name, relative to the CRL issuer.
    NameRelativeToCrlIssuer,
    /// The distribution point name is a sequence of [GeneralName] items.
    FullName(DerIterator<'a, GeneralName<'a>>),
}

impl<'a> FromDer<'a> for DistributionPointName<'a> {
    fn from_der(reader: &mut untrusted::Reader<'a>) -> Result<Self, Error> {
        // RFC 5280 section §4.2.1.13:
        //   When the distributionPoint field is present, it contains either a
        //   SEQUENCE of general names or a single value, nameRelativeToCRLIssuer
        const FULL_NAME_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED;
        const NAME_RELATIVE_TO_CRL_ISSUER_TAG: u8 = CONTEXT_SPECIFIC | CONSTRUCTED | 1;

        let (tag, value) = der::read_tag_and_get_value(reader)?;
        match tag {
            FULL_NAME_TAG => Ok(DistributionPointName::FullName(DerIterator::new(value))),
            NAME_RELATIVE_TO_CRL_ISSUER_TAG => Ok(DistributionPointName::NameRelativeToCrlIssuer),
            _ => Err(Error::BadDer),
        }
    }

    const TYPE_ID: DerTypeId = DerTypeId::DistributionPointName;
}
