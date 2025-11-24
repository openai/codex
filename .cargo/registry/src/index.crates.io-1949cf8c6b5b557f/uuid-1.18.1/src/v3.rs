use crate::Uuid;

impl Uuid {
    /// Creates a UUID using a name from a namespace, based on the MD5
    /// hash.
    ///
    /// A number of namespaces are available as constants in this crate:
    ///
    /// * [`NAMESPACE_DNS`]
    /// * [`NAMESPACE_OID`]
    /// * [`NAMESPACE_URL`]
    /// * [`NAMESPACE_X500`]
    ///
    /// Note that usage of this method requires the `v3` feature of this crate
    /// to be enabled.
    ///
    /// # Examples
    ///
    /// Generating a MD5 DNS UUID for `rust-lang.org`:
    ///
    /// ```
    /// # use uuid::{Uuid, Version};
    /// let uuid = Uuid::new_v3(&Uuid::NAMESPACE_DNS, b"rust-lang.org");
    ///
    /// assert_eq!(Some(Version::Md5), uuid.get_version());
    /// ```
    ///
    /// # References
    ///
    /// * [UUID Version 3 in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-5.3)
    /// * [Name-Based UUID Generation in RFC 9562](https://www.ietf.org/rfc/rfc9562.html#section-6.5)
    ///
    /// [`NAMESPACE_DNS`]: #associatedconstant.NAMESPACE_DNS
    /// [`NAMESPACE_OID`]: #associatedconstant.NAMESPACE_OID
    /// [`NAMESPACE_URL`]: #associatedconstant.NAMESPACE_URL
    /// [`NAMESPACE_X500`]: #associatedconstant.NAMESPACE_X500
    pub fn new_v3(namespace: &Uuid, name: &[u8]) -> Uuid {
        crate::Builder::from_md5_bytes(crate::md5::hash(namespace.as_bytes(), name)).into_uuid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::*;

    use crate::{std::string::ToString, Variant, Version};

    static FIXTURE: &'static [(&'static Uuid, &'static str, &'static str)] = &[
        (
            &Uuid::NAMESPACE_DNS,
            "example.org",
            "04738bdf-b25a-3829-a801-b21a1d25095b",
        ),
        (
            &Uuid::NAMESPACE_DNS,
            "rust-lang.org",
            "c6db027c-615c-3b4d-959e-1a917747ca5a",
        ),
        (
            &Uuid::NAMESPACE_DNS,
            "42",
            "5aab6e0c-b7d3-379c-92e3-2bfbb5572511",
        ),
        (
            &Uuid::NAMESPACE_DNS,
            "lorem ipsum",
            "4f8772e9-b59c-3cc9-91a9-5c823df27281",
        ),
        (
            &Uuid::NAMESPACE_URL,
            "example.org",
            "39682ca1-9168-3da2-a1bb-f4dbcde99bf9",
        ),
        (
            &Uuid::NAMESPACE_URL,
            "rust-lang.org",
            "7ed45aaf-e75b-3130-8e33-ee4d9253b19f",
        ),
        (
            &Uuid::NAMESPACE_URL,
            "42",
            "08998a0c-fcf4-34a9-b444-f2bfc15731dc",
        ),
        (
            &Uuid::NAMESPACE_URL,
            "lorem ipsum",
            "e55ad2e6-fb89-34e8-b012-c5dde3cd67f0",
        ),
        (
            &Uuid::NAMESPACE_OID,
            "example.org",
            "f14eec63-2812-3110-ad06-1625e5a4a5b2",
        ),
        (
            &Uuid::NAMESPACE_OID,
            "rust-lang.org",
            "6506a0ec-4d79-3e18-8c2b-f2b6b34f2b6d",
        ),
        (
            &Uuid::NAMESPACE_OID,
            "42",
            "ce6925a5-2cd7-327b-ab1c-4b375ac044e4",
        ),
        (
            &Uuid::NAMESPACE_OID,
            "lorem ipsum",
            "5dd8654f-76ba-3d47-bc2e-4d6d3a78cb09",
        ),
        (
            &Uuid::NAMESPACE_X500,
            "example.org",
            "64606f3f-bd63-363e-b946-fca13611b6f7",
        ),
        (
            &Uuid::NAMESPACE_X500,
            "rust-lang.org",
            "bcee7a9c-52f1-30c6-a3cc-8c72ba634990",
        ),
        (
            &Uuid::NAMESPACE_X500,
            "42",
            "c1073fa2-d4a6-3104-b21d-7a6bdcf39a23",
        ),
        (
            &Uuid::NAMESPACE_X500,
            "lorem ipsum",
            "02f09a3f-1624-3b1d-8409-44eff7708208",
        ),
    ];

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_new() {
        for &(ref ns, ref name, _) in FIXTURE {
            let uuid = Uuid::new_v3(*ns, name.as_bytes());
            assert_eq!(uuid.get_version(), Some(Version::Md5));
            assert_eq!(uuid.get_variant(), Variant::RFC4122);
        }
    }

    #[test]
    #[cfg_attr(
        all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")),
        wasm_bindgen_test
    )]
    fn test_hyphenated_string() {
        for &(ref ns, ref name, ref expected) in FIXTURE {
            let uuid = Uuid::new_v3(*ns, name.as_bytes());
            assert_eq!(uuid.hyphenated().to_string(), *expected);
        }
    }
}
