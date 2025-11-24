use crate::{Builder, Uuid};

impl Uuid {
    /// Creates a custom UUID comprised almost entirely of user-supplied bytes.
    ///
    /// This will inject the UUID Version at 4 bits starting at the 48th bit
    /// and the Variant into 2 bits 64th bit. Any existing bits in the user-supplied bytes
    /// at those locations will be overridden.
    ///
    /// Note that usage of this method requires the `v8` feature of this crate
    /// to be enabled.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # use uuid::{Uuid, Version};
    /// let buf: [u8; 16] = *b"abcdefghijklmnop";
    /// let uuid = Uuid::new_v8(buf);
    ///
    /// assert_eq!(Some(Version::Custom), uuid.get_version());
    /// ```
    ///
    /// # References
    ///
    /// * [UUID Version 8 in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-5.8)
    pub const fn new_v8(buf: [u8; 16]) -> Uuid {
        Builder::from_custom_bytes(buf).into_uuid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Variant, Version};
    use std::string::ToString;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::*;

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_new() {
        let buf: [u8; 16] = [
            0xf, 0xe, 0xd, 0xc, 0xb, 0xa, 0x9, 0x8, 0x7, 0x6, 0x5, 0x4, 0x3, 0x2, 0x1, 0x0,
        ];
        let uuid = Uuid::new_v8(buf);
        assert_eq!(uuid.get_version(), Some(Version::Custom));
        assert_eq!(uuid.get_variant(), Variant::RFC4122);
        assert_eq!(uuid.get_version_num(), 8);
        assert_eq!(
            uuid.hyphenated().to_string(),
            "0f0e0d0c-0b0a-8908-8706-050403020100"
        );
    }
}
