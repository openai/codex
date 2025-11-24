#[cfg(not(feature = "reset"))]
use digest::new_mac_test as test;
#[cfg(feature = "reset")]
use digest::new_resettable_mac_test as test;
use hmac::{Hmac, SimpleHmac};
use sha1::Sha1;
use sha2::{Sha224, Sha256, Sha384, Sha512};
use streebog::{Streebog256, Streebog512};

// Test vectors from RFC 2104, plus wiki test
test!(hmac_md5_rfc2104, "md5", Hmac<md5::Md5>);
test!(hmac_md5_rfc2104_simple, "md5", SimpleHmac<md5::Md5>);

// Test vectors from RFC 4231
test!(hmac_sha224_rfc4231, "sha224", Hmac<Sha224>);
test!(hmac_sha256_rfc4231, "sha256", Hmac<Sha256>);
test!(hmac_sha384_rfc4231, "sha384", Hmac<Sha384>);
test!(hmac_sha512_rfc4231, "sha512", Hmac<Sha512>);
test!(hmac_sha224_rfc4231_simple, "sha224", SimpleHmac<Sha224>);
test!(hmac_sha256_rfc4231_simple, "sha256", SimpleHmac<Sha256>);
test!(hmac_sha384_rfc4231_simple, "sha384", SimpleHmac<Sha384>);
test!(hmac_sha512_rfc4231_simple, "sha512", SimpleHmac<Sha512>);

// Test vectors from R 50.1.113-2016:
// https://tc26.ru/standard/rs/Р 50.1.113-2016.pdf
test!(hmac_streebog256, "streebog256", Hmac<Streebog256>);
test!(hmac_streebog512, "streebog512", Hmac<Streebog512>);
test!(
    hmac_streebog256_simple,
    "streebog256",
    SimpleHmac<Streebog256>
);
test!(
    hmac_streebog512_simple,
    "streebog512",
    SimpleHmac<Streebog512>
);

// Tests from Project Wycheproof:
// https://github.com/google/wycheproof
test!(
    hmac_sha1_wycheproof,
    "wycheproof-sha1",
    Hmac<Sha1>,
    trunc_left,
);
test!(
    hmac_sha256_wycheproof,
    "wycheproof-sha256",
    Hmac<Sha256>,
    trunc_left,
);
test!(
    hmac_sha384_wycheproof,
    "wycheproof-sha384",
    Hmac<Sha384>,
    trunc_left,
);
test!(
    hmac_sha512_wycheproof,
    "wycheproof-sha512",
    Hmac<Sha512>,
    trunc_left,
);
test!(
    hmac_sha1_wycheproof_simple,
    "wycheproof-sha1",
    SimpleHmac<Sha1>,
    trunc_left,
);
test!(
    hmac_sha256_wycheproof_simple,
    "wycheproof-sha256",
    SimpleHmac<Sha256>,
    trunc_left,
);
test!(
    hmac_sha384_wycheproof_simple,
    "wycheproof-sha384",
    SimpleHmac<Sha384>,
    trunc_left,
);
test!(
    hmac_sha512_wycheproof_simple,
    "wycheproof-sha512",
    SimpleHmac<Sha512>,
    trunc_left,
);
