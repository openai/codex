//! Base64 encoding support.
use crate::error::ErrorStack;
use crate::{cvt_n, LenType};
use libc::c_int;
use openssl_macros::corresponds;

/// Encodes a slice of bytes to a base64 string.
///
/// # Panics
///
/// Panics if the input length or computed output length overflow a signed C integer.
#[corresponds(EVP_EncodeBlock)]
pub fn encode_block(src: &[u8]) -> String {
    assert!(src.len() <= c_int::MAX as usize);
    let src_len = src.len() as LenType;

    let len = encoded_len(src_len).unwrap();
    let mut out = Vec::with_capacity(len as usize);

    // SAFETY: `encoded_len` ensures space for 4 output characters
    // for every 3 input bytes including padding and nul terminator.
    // `EVP_EncodeBlock` will write only single byte ASCII characters.
    // `EVP_EncodeBlock` will only write to not read from `out`.
    unsafe {
        let out_len = ffi::EVP_EncodeBlock(out.as_mut_ptr(), src.as_ptr(), src_len);
        out.set_len(out_len as usize);
        String::from_utf8_unchecked(out)
    }
}

/// Decodes a base64-encoded string to bytes.
///
/// # Panics
///
/// Panics if the input length or computed output length overflow a signed C integer.
#[corresponds(EVP_DecodeBlock)]
pub fn decode_block(src: &str) -> Result<Vec<u8>, ErrorStack> {
    let src = src.trim();

    // https://github.com/openssl/openssl/issues/12143
    if src.is_empty() {
        return Ok(vec![]);
    }

    assert!(src.len() <= c_int::MAX as usize);
    let src_len = src.len() as LenType;

    let len = decoded_len(src_len).unwrap();
    let mut out = Vec::with_capacity(len as usize);

    // SAFETY: `decoded_len` ensures space for 3 output bytes
    // for every 4 input characters including padding.
    // `EVP_DecodeBlock` can write fewer bytes after stripping
    // leading and trailing whitespace, but never more.
    // `EVP_DecodeBlock` will only write to not read from `out`.
    unsafe {
        let out_len = cvt_n(ffi::EVP_DecodeBlock(
            out.as_mut_ptr(),
            src.as_ptr(),
            src_len,
        ))?;
        out.set_len(out_len as usize);
    }

    if src.ends_with('=') {
        out.pop();
        if src.ends_with("==") {
            out.pop();
        }
    }

    Ok(out)
}

fn encoded_len(src_len: LenType) -> Option<LenType> {
    let mut len = (src_len / 3).checked_mul(4)?;

    if src_len % 3 != 0 {
        len = len.checked_add(4)?;
    }

    len = len.checked_add(1)?;

    Some(len)
}

fn decoded_len(src_len: LenType) -> Option<LenType> {
    let mut len = (src_len / 4).checked_mul(3)?;

    if src_len % 4 != 0 {
        len = len.checked_add(3)?;
    }

    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_block() {
        assert_eq!("".to_string(), encode_block(b""));
        assert_eq!("Zg==".to_string(), encode_block(b"f"));
        assert_eq!("Zm8=".to_string(), encode_block(b"fo"));
        assert_eq!("Zm9v".to_string(), encode_block(b"foo"));
        assert_eq!("Zm9vYg==".to_string(), encode_block(b"foob"));
        assert_eq!("Zm9vYmE=".to_string(), encode_block(b"fooba"));
        assert_eq!("Zm9vYmFy".to_string(), encode_block(b"foobar"));
    }

    #[test]
    fn test_decode_block() {
        assert_eq!(b"".to_vec(), decode_block("").unwrap());
        assert_eq!(b"f".to_vec(), decode_block("Zg==").unwrap());
        assert_eq!(b"fo".to_vec(), decode_block("Zm8=").unwrap());
        assert_eq!(b"foo".to_vec(), decode_block("Zm9v").unwrap());
        assert_eq!(b"foob".to_vec(), decode_block("Zm9vYg==").unwrap());
        assert_eq!(b"fooba".to_vec(), decode_block("Zm9vYmE=").unwrap());
        assert_eq!(b"foobar".to_vec(), decode_block("Zm9vYmFy").unwrap());
    }

    #[test]
    fn test_strip_whitespace() {
        assert_eq!(b"foobar".to_vec(), decode_block(" Zm9vYmFy\n").unwrap());
        assert_eq!(b"foob".to_vec(), decode_block(" Zm9vYg==\n").unwrap());
    }
}
