// Copyright 2015-2020 Brian Smith.
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
use alloc::format;

use pki_types::IpAddr;
#[cfg(feature = "alloc")]
use pki_types::ServerName;

use super::{GeneralName, NameIterator};
use crate::cert::Cert;
use crate::error::{Error, InvalidNameContext};

pub(crate) fn verify_ip_address_names(reference: &IpAddr, cert: &Cert<'_>) -> Result<(), Error> {
    let ip_address = match reference {
        IpAddr::V4(ip) => untrusted::Input::from(ip.as_ref()),
        IpAddr::V6(ip) => untrusted::Input::from(ip.as_ref()),
    };

    let result = NameIterator::new(cert.subject_alt_name).find_map(|result| {
        let name = match result {
            Ok(name) => name,
            Err(err) => return Some(Err(err)),
        };

        let presented_id = match name {
            GeneralName::IpAddress(presented) => presented,
            _ => return None,
        };

        match presented_id_matches_reference_id(presented_id, ip_address) {
            true => Some(Ok(())),
            false => None,
        }
    });

    match result {
        Some(result) => return result,
        #[cfg(feature = "alloc")]
        None => {}
        #[cfg(not(feature = "alloc"))]
        None => Err(Error::CertNotValidForName(InvalidNameContext {})),
    }

    #[cfg(feature = "alloc")]
    {
        Err(Error::CertNotValidForName(InvalidNameContext {
            expected: ServerName::from(*reference),
            presented: NameIterator::new(cert.subject_alt_name)
                .filter_map(|result| Some(format!("{:?}", result.ok()?)))
                .collect(),
        }))
    }
}

// https://tools.ietf.org/html/rfc5280#section-4.2.1.6 says:
//   When the subjectAltName extension contains an iPAddress, the address
//   MUST be stored in the octet string in "network byte order", as
//   specified in [RFC791].  The least significant bit (LSB) of each octet
//   is the LSB of the corresponding byte in the network address.  For IP
//   version 4, as specified in [RFC791], the octet string MUST contain
//   exactly four octets.  For IP version 6, as specified in
//   [RFC2460], the octet string MUST contain exactly sixteen octets.
fn presented_id_matches_reference_id(
    presented_id: untrusted::Input<'_>,
    reference_id: untrusted::Input<'_>,
) -> bool {
    match (presented_id.len(), reference_id.len()) {
        (4, 4) => (),
        (16, 16) => (),
        _ => {
            return false;
        }
    };

    let mut presented_ip_address = untrusted::Reader::new(presented_id);
    let mut reference_ip_address = untrusted::Reader::new(reference_id);
    while !presented_ip_address.at_end() {
        let presented_ip_address_byte = presented_ip_address.read_byte().unwrap();
        let reference_ip_address_byte = reference_ip_address.read_byte().unwrap();
        if presented_ip_address_byte != reference_ip_address_byte {
            return false;
        }
    }

    true
}

// https://tools.ietf.org/html/rfc5280#section-4.2.1.10 says:
//
//     For IPv4 addresses, the iPAddress field of GeneralName MUST contain
//     eight (8) octets, encoded in the style of RFC 4632 (CIDR) to represent
//     an address range [RFC4632].  For IPv6 addresses, the iPAddress field
//     MUST contain 32 octets similarly encoded.  For example, a name
//     constraint for "class C" subnet 192.0.2.0 is represented as the
//     octets C0 00 02 00 FF FF FF 00, representing the CIDR notation
//     192.0.2.0/24 (mask 255.255.255.0).
pub(super) fn presented_id_matches_constraint(
    name: untrusted::Input<'_>,
    constraint: untrusted::Input<'_>,
) -> Result<bool, Error> {
    match (name.len(), constraint.len()) {
        (4, 8) => (),
        (16, 32) => (),

        // an IPv4 address never matches an IPv6 constraint, and vice versa.
        (4, 32) | (16, 8) => {
            return Ok(false);
        }

        // invalid constraint length
        (4, _) | (16, _) => {
            return Err(Error::InvalidNetworkMaskConstraint);
        }

        // invalid name length, or anything else
        _ => {
            return Err(Error::BadDer);
        }
    };

    let (constraint_address, constraint_mask) = constraint.read_all(Error::BadDer, |value| {
        let address = value.read_bytes(constraint.len() / 2).unwrap();
        let mask = value.read_bytes(constraint.len() / 2).unwrap();
        Ok((address, mask))
    })?;

    let mut name = untrusted::Reader::new(name);
    let mut constraint_address = untrusted::Reader::new(constraint_address);
    let mut constraint_mask = untrusted::Reader::new(constraint_mask);
    let mut seen_zero_bit = false;

    loop {
        // Iterate through the name, constraint address, and constraint mask
        // a byte at a time.
        let name_byte = name.read_byte().unwrap();
        let constraint_address_byte = constraint_address.read_byte().unwrap();
        let constraint_mask_byte = constraint_mask.read_byte().unwrap();

        // A valid mask consists of a sequence of 1 bits, followed by a
        // sequence of 0 bits.  Either sequence could be empty.

        let leading = constraint_mask_byte.leading_ones();
        let trailing = constraint_mask_byte.trailing_zeros();

        // At the resolution of a single octet, a valid mask is one where
        // leading_ones() and trailing_zeros() sums to 8.
        // This includes all-ones and all-zeroes.
        if leading + trailing != 8 {
            return Err(Error::InvalidNetworkMaskConstraint);
        }

        // There should be no bits set after the first octet with a zero bit is seen.
        if seen_zero_bit && constraint_mask_byte != 0x00 {
            return Err(Error::InvalidNetworkMaskConstraint);
        }

        // Note when a zero bit is seen for later octets.
        if constraint_mask_byte != 0xff {
            seen_zero_bit = true;
        }

        if ((name_byte ^ constraint_address_byte) & constraint_mask_byte) != 0 {
            return Ok(false);
        }
        if name.at_end() {
            break;
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presented_id_matches_constraint_ipv4_test() {
        let names_and_constraints = vec![
            (
                // 192.0.2.0 matches constraint 192.0.2.0/24
                [0xC0, 0x00, 0x02, 0x00],
                [0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00],
                Ok(true),
            ),
            (
                // 192.0.2.1 matches constraint 192.0.2.0/24
                [0xC0, 0x00, 0x02, 0x01],
                [0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00],
                Ok(true),
            ),
            (
                // 192.0.2.255 matches constraint 192.0.2.0/24
                [0xC0, 0x00, 0x02, 0xFF],
                [0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00],
                Ok(true),
            ),
            (
                // 192.0.1.255 does not match constraint 192.0.2.0/24
                [0xC0, 0x00, 0x01, 0xFF],
                [0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00],
                Ok(false),
            ),
            (
                // 192.0.3.0 does not match constraint 192.0.2.0/24
                [0xC0, 0x00, 0x03, 0x00],
                [0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00],
                Ok(false),
            ),
        ];
        for (name, constraint, match_result) in names_and_constraints {
            assert_eq!(
                presented_id_matches_constraint(
                    untrusted::Input::from(&name),
                    untrusted::Input::from(&constraint),
                ),
                match_result
            )
        }

        // Invalid name length (shorter)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[0xC0, 0x00, 0x02]),
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00]),
            ),
            Err(Error::BadDer),
        );

        // Invalid name length (longer)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00, 0x00]),
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00]),
            ),
            Err(Error::BadDer),
        );

        // Unmatching constraint size (shorter)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00]),
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF]),
            ),
            Err(Error::InvalidNetworkMaskConstraint),
        );

        // Unmatching constraint size (longer)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00]),
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x00]),
            ),
            Err(Error::InvalidNetworkMaskConstraint),
        );

        // Unmatching constraint size (IPv6 constraint for IPv4 address)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00]),
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00
                ]),
            ),
            Ok(false),
        );
    }

    #[test]
    fn presented_id_matches_constraint_ipv6_test() {
        let names_and_constraints = vec![
            (
                // 2001:0DB8:ABCD:0012:0000:0000:0000:0000 matches constraint
                //   2001:0DB8:ABCD:0012:0000:0000:0000:0000/64
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00,
                ],
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                Ok(true),
            ),
            (
                // 2001:0DB8:ABCD:0012:0000:0000:0000:0001 matches constraint
                //   2001:0DB8:ABCD:0012:0000:0000:0000:0000/64
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x01,
                ],
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                Ok(true),
            ),
            (
                // 2001:0DB8:ABCD:0012:FFFF:FFFF:FFFF:FFFF matches constraint
                //   2001:0DB8:ABCD:0012:0000:0000:0000:0000/64
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                    0xFF, 0xFF, 0xFF,
                ],
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                Ok(true),
            ),
            (
                // 2001:0DB8:ABCD:0011:0000:0000:0000:0000 does not match constraint
                //   2001:0DB8:ABCD:0012:0000:0000:0000:0000/64
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00,
                ],
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                Ok(false),
            ),
            (
                // 2001:0DB8:ABCD:0013:0000:0000:0000:0000 does not match constraint
                //   2001:0DB8:ABCD:0012:0000:0000:0000:0000/64
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00,
                ],
                [
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                Ok(false),
            ),
        ];
        for (name, constraint, match_result) in names_and_constraints {
            assert_eq!(
                presented_id_matches_constraint(
                    untrusted::Input::from(&name),
                    untrusted::Input::from(&constraint),
                ),
                match_result
            )
        }

        // Invalid name length (shorter)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00
                ]),
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00
                ]),
            ),
            Err(Error::BadDer),
        );

        // Invalid name length (longer)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00
                ]),
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00
                ]),
            ),
            Err(Error::BadDer),
        );

        // Unmatching constraint size (shorter)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00,
                ]),
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00
                ]),
            ),
            Err(Error::InvalidNetworkMaskConstraint),
        );

        // Unmatching constraint size (longer)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00,
                ]),
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
                ]),
            ),
            Err(Error::InvalidNetworkMaskConstraint),
        );

        // Unmatching constraint size (IPv4 constraint for IPv6 address)
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(&[
                    0x20, 0x01, 0x0D, 0xB8, 0xAB, 0xCD, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00,
                ]),
                untrusted::Input::from(&[0xC0, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF, 0x00]),
            ),
            Ok(false),
        );
    }

    #[test]
    fn test_presented_id_matches_reference_id() {
        assert!(!presented_id_matches_reference_id(
            untrusted::Input::from(&[]),
            untrusted::Input::from(&[]),
        ));

        assert!(!presented_id_matches_reference_id(
            untrusted::Input::from(&[0x01]),
            untrusted::Input::from(&[])
        ));

        assert!(!presented_id_matches_reference_id(
            untrusted::Input::from(&[]),
            untrusted::Input::from(&[0x01])
        ));

        assert!(presented_id_matches_reference_id(
            untrusted::Input::from(&[1, 2, 3, 4]),
            untrusted::Input::from(&[1, 2, 3, 4])
        ));

        assert!(!presented_id_matches_reference_id(
            untrusted::Input::from(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
            untrusted::Input::from(&[1, 2, 3, 4])
        ));

        assert!(!presented_id_matches_reference_id(
            untrusted::Input::from(&[1, 2, 3, 4]),
            untrusted::Input::from(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
        ));

        assert!(presented_id_matches_reference_id(
            untrusted::Input::from(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
            untrusted::Input::from(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
        ));
    }

    #[test]
    fn presented_id_matches_constraint_rejects_incorrect_length_arguments() {
        // wrong length names
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(b"\x00\x00\x00"),
                untrusted::Input::from(b"")
            ),
            Err(Error::BadDer)
        );
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(b"\x00\x00\x00\x00\x00"),
                untrusted::Input::from(b"")
            ),
            Err(Error::BadDer)
        );

        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(
                    b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
                ),
                untrusted::Input::from(b"")
            ),
            Err(Error::BadDer)
        );
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(
                    b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
                ),
                untrusted::Input::from(b"")
            ),
            Err(Error::BadDer)
        );

        // wrong length constraints
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(b"\x00\x00\x00\x00"),
                untrusted::Input::from(b"\x00\x00\x00\x00\xff\xff\xff")
            ),
            Err(Error::InvalidNetworkMaskConstraint)
        );
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(b"\x00\x00\x00\x00"),
                untrusted::Input::from(b"\x00\x00\x00\x00\xff\xff\xff\xff\x00")
            ),
            Err(Error::InvalidNetworkMaskConstraint)
        );
        assert_eq!(
            presented_id_matches_constraint(untrusted::Input::from(b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"),
                                            untrusted::Input::from(b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\
                                                                     \xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff")),
            Err(Error::InvalidNetworkMaskConstraint)
        );
        assert_eq!(
            presented_id_matches_constraint(untrusted::Input::from(b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"),
                                            untrusted::Input::from(b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\
                                                                     \xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff")),
            Err(Error::InvalidNetworkMaskConstraint)
        );

        // ipv4-length not considered for ipv6-length name, and vv
        assert_eq!(
            presented_id_matches_constraint(untrusted::Input::from(b"\x00\x00\x00\x00"),
                                            untrusted::Input::from(b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\
                                                                     \xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff")),
            Ok(false)
        );
        assert_eq!(
            presented_id_matches_constraint(
                untrusted::Input::from(
                    b"\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
                ),
                untrusted::Input::from(b"\x00\x00\x00\x00\xff\xff\xff\xff")
            ),
            Ok(false)
        );
    }
}

#[cfg(all(test, feature = "std"))]
mod alloc_tests {
    use super::*;

    // (presented_address, constraint_address, constraint_mask, expected_result)
    const PRESENTED_MATCHES_CONSTRAINT: &[(&str, &str, &str, Result<bool, Error>)] = &[
        // Cannot mix IpV4 with IpV6 and viceversa
        ("2001:db8::", "8.8.8.8", "255.255.255.255", Ok(false)),
        ("8.8.8.8", "2001:db8::", "ffff::", Ok(false)),
        // IpV4 non-contiguous masks
        (
            "8.8.8.8",
            "8.8.8.8",
            "255.255.255.1",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "8.8.8.8",
            "8.8.8.8",
            "255.255.0.255",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "8.8.8.8",
            "8.8.8.8",
            "255.0.255.255",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "8.8.8.8",
            "8.8.8.8",
            "0.255.255.255",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "8.8.8.8",
            "8.8.8.8",
            "1.255.255.255",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "8.8.8.8",
            "8.8.8.8",
            "128.128.128.128",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        // IpV4
        ("8.8.8.8", "8.8.8.8", "255.255.255.255", Ok(true)),
        ("8.8.8.9", "8.8.8.8", "255.255.255.255", Ok(false)),
        ("8.8.8.9", "8.8.8.8", "255.255.255.254", Ok(true)),
        ("8.8.8.10", "8.8.8.8", "255.255.255.254", Ok(false)),
        ("8.8.8.10", "8.8.8.8", "255.255.255.0", Ok(true)),
        ("8.8.15.10", "8.8.8.8", "255.255.248.0", Ok(true)),
        ("8.8.16.10", "8.8.8.8", "255.255.248.0", Ok(false)),
        ("8.8.16.10", "8.8.8.8", "255.255.0.0", Ok(true)),
        ("8.31.16.10", "8.8.8.8", "255.224.0.0", Ok(true)),
        ("8.32.16.10", "8.8.8.8", "255.224.0.0", Ok(false)),
        ("8.32.16.10", "8.8.8.8", "255.0.0.0", Ok(true)),
        ("63.32.16.10", "8.8.8.8", "192.0.0.0", Ok(true)),
        ("64.32.16.10", "8.8.8.8", "192.0.0.0", Ok(false)),
        ("64.32.16.10", "8.8.8.8", "0.0.0.0", Ok(true)),
        // IpV6 non-contiguous masks
        (
            "2001:db8::",
            "2001:db8::",
            "fffe:ffff::",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "2001:db8::",
            "2001:db8::",
            "ffff:fdff::",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "2001:db8::",
            "2001:db8::",
            "ffff:feff::",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "2001:db8::",
            "2001:db8::",
            "ffff:fcff::",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        (
            "2001:db8::",
            "2001:db8::",
            "7fff:ffff::",
            Err(Error::InvalidNetworkMaskConstraint),
        ),
        // IpV6
        ("2001:db8::", "2001:db8::", "ffff:ffff::", Ok(true)),
        ("2001:db9::", "2001:db8::", "ffff:ffff::", Ok(false)),
        ("2001:db9::", "2001:db8::", "ffff:fffe::", Ok(true)),
        ("2001:dba::", "2001:db8::", "ffff:fffe::", Ok(false)),
        ("2001:dba::", "2001:db8::", "ffff:ff00::", Ok(true)),
        ("2001:dca::", "2001:db8::", "ffff:fe00::", Ok(true)),
        ("2001:fca::", "2001:db8::", "ffff:fe00::", Ok(false)),
        ("2001:fca::", "2001:db8::", "ffff:0000::", Ok(true)),
        ("2000:fca::", "2001:db8::", "fffe:0000::", Ok(true)),
        ("2003:fca::", "2001:db8::", "fffe:0000::", Ok(false)),
        ("2003:fca::", "2001:db8::", "ff00:0000::", Ok(true)),
        ("1003:fca::", "2001:db8::", "e000:0000::", Ok(false)),
        ("1003:fca::", "2001:db8::", "0000:0000::", Ok(true)),
    ];

    #[test]
    fn presented_matches_constraint_test() {
        use std::boxed::Box;
        use std::net::IpAddr;

        for (presented, constraint_address, constraint_mask, expected_result) in
            PRESENTED_MATCHES_CONSTRAINT
        {
            let presented_bytes: Box<[u8]> = match presented.parse::<IpAddr>().unwrap() {
                IpAddr::V4(p) => Box::new(p.octets()),
                IpAddr::V6(p) => Box::new(p.octets()),
            };
            let ca_bytes: Box<[u8]> = match constraint_address.parse::<IpAddr>().unwrap() {
                IpAddr::V4(ca) => Box::new(ca.octets()),
                IpAddr::V6(ca) => Box::new(ca.octets()),
            };
            let cm_bytes: Box<[u8]> = match constraint_mask.parse::<IpAddr>().unwrap() {
                IpAddr::V4(cm) => Box::new(cm.octets()),
                IpAddr::V6(cm) => Box::new(cm.octets()),
            };
            let constraint_bytes = [ca_bytes, cm_bytes].concat();
            let actual_result = presented_id_matches_constraint(
                untrusted::Input::from(&presented_bytes),
                untrusted::Input::from(&constraint_bytes),
            );
            assert_eq!(
                &actual_result, expected_result,
                "presented_id_matches_constraint(\"{presented_bytes:?}\", \"{constraint_bytes:?}\")"
            );
        }
    }
}
