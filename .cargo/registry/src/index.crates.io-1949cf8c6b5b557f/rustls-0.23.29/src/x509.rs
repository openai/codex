// Additional x509/asn1 functions to those provided in webpki/ring.

use alloc::vec::Vec;

/// Prepend stuff to `bytes` to put it in a DER SEQUENCE.
pub(crate) fn wrap_in_sequence(bytes: &[u8]) -> Vec<u8> {
    asn1_wrap(DER_SEQUENCE_TAG, bytes, &[])
}

/// Prepend stuff to `bytes_a` + `bytes_b` to put it in a DER SEQUENCE.
#[cfg_attr(not(feature = "ring"), allow(dead_code))]
pub(crate) fn wrap_concat_in_sequence(bytes_a: &[u8], bytes_b: &[u8]) -> Vec<u8> {
    asn1_wrap(DER_SEQUENCE_TAG, bytes_a, bytes_b)
}

/// Prepend stuff to `bytes` to put it in a DER BIT STRING.
pub(crate) fn wrap_in_bit_string(bytes: &[u8]) -> Vec<u8> {
    asn1_wrap(DER_BIT_STRING_TAG, &[0u8], bytes)
}

/// Prepend stuff to `bytes` to put it in a DER OCTET STRING.
#[cfg_attr(not(feature = "ring"), allow(dead_code))]
pub(crate) fn wrap_in_octet_string(bytes: &[u8]) -> Vec<u8> {
    asn1_wrap(DER_OCTET_STRING_TAG, bytes, &[])
}

fn asn1_wrap(tag: u8, bytes_a: &[u8], bytes_b: &[u8]) -> Vec<u8> {
    let len = bytes_a.len() + bytes_b.len();

    if len <= 0x7f {
        // Short form
        let mut ret = Vec::with_capacity(2 + len);
        ret.push(tag);
        ret.push(len as u8);
        ret.extend_from_slice(bytes_a);
        ret.extend_from_slice(bytes_b);
        ret
    } else {
        // Long form
        let size = len.to_be_bytes();
        let leading_zero_bytes = size
            .iter()
            .position(|&x| x != 0)
            .unwrap_or(size.len());
        assert!(leading_zero_bytes < size.len());
        let encoded_bytes = size.len() - leading_zero_bytes;

        let mut ret = Vec::with_capacity(2 + encoded_bytes + len);
        ret.push(tag);

        ret.push(0x80 + encoded_bytes as u8);
        ret.extend_from_slice(&size[leading_zero_bytes..]);

        ret.extend_from_slice(bytes_a);
        ret.extend_from_slice(bytes_b);
        ret
    }
}

const DER_SEQUENCE_TAG: u8 = 0x30;
const DER_BIT_STRING_TAG: u8 = 0x03;
const DER_OCTET_STRING_TAG: u8 = 0x04;

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[test]
    fn test_empty() {
        assert_eq!(vec![0x30, 0x00], wrap_in_sequence(&[]));
    }

    #[test]
    fn test_small() {
        assert_eq!(
            vec![0x30, 0x04, 0x00, 0x11, 0x22, 0x33],
            wrap_in_sequence(&[0x00, 0x11, 0x22, 0x33])
        );
    }

    #[test]
    fn test_medium() {
        let mut val = Vec::new();
        val.resize(255, 0x12);
        assert_eq!(
            vec![0x30, 0x81, 0xff, 0x12, 0x12, 0x12],
            wrap_in_sequence(&val)[..6]
        );
    }

    #[test]
    fn test_large() {
        let mut val = Vec::new();
        val.resize(4660, 0x12);
        wrap_in_sequence(&val);
        assert_eq!(
            vec![0x30, 0x82, 0x12, 0x34, 0x12, 0x12],
            wrap_in_sequence(&val)[..6]
        );
    }

    #[test]
    fn test_huge() {
        let mut val = Vec::new();
        val.resize(0xffff, 0x12);
        let result = wrap_in_sequence(&val);
        assert_eq!(vec![0x30, 0x82, 0xff, 0xff, 0x12, 0x12], result[..6]);
        assert_eq!(result.len(), 0xffff + 4);
    }

    #[test]
    fn test_gigantic() {
        let mut val = Vec::new();
        val.resize(0x100000, 0x12);
        let result = wrap_in_sequence(&val);
        assert_eq!(vec![0x30, 0x83, 0x10, 0x00, 0x00, 0x12, 0x12], result[..7]);
        assert_eq!(result.len(), 0x100000 + 5);
    }

    #[test]
    fn test_ludicrous() {
        let mut val = Vec::new();
        val.resize(0x1000000, 0x12);
        let result = wrap_in_sequence(&val);
        assert_eq!(
            vec![0x30, 0x84, 0x01, 0x00, 0x00, 0x00, 0x12, 0x12],
            result[..8]
        );
        assert_eq!(result.len(), 0x1000000 + 6);
    }

    #[test]
    fn test_wrap_in_bit_string() {
        // The BIT STRING encoding starts with a single octet on
        // the front saying how many bits to disregard from the
        // last octet. So this zero means "no bits" unused, which
        // is correct because our input is an string of octets.
        //
        // So if we encode &[0x55u8] with this function, we should get:
        //
        // 0x03    0x02    0x00                0x55
        // ^ tag   ^ len   ^ no unused bits    ^ value
        assert_eq!(wrap_in_bit_string(&[0x55u8]), vec![0x03, 0x02, 0x00, 0x55]);
    }
}
