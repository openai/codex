use rustls_pki_types::PrivateKeyDer;

#[test]
fn test_private_key_from_der() {
    const NIST_P256_KEY_SEC1: &[u8] = include_bytes!("../tests/keys/nistp256key.der");
    const NIST_P384_KEY_SEC1: &[u8] = include_bytes!("../tests/keys/nistp384key.der");
    const NIST_P521_KEY_SEC1: &[u8] = include_bytes!("../tests/keys/nistp521key.der");
    for bytes in [NIST_P256_KEY_SEC1, NIST_P384_KEY_SEC1, NIST_P521_KEY_SEC1] {
        assert!(matches!(
            PrivateKeyDer::try_from(bytes).unwrap(),
            PrivateKeyDer::Sec1(_)
        ));
    }

    const RSA_2048_KEY_PKCS1: &[u8] = include_bytes!("../tests/keys/rsa2048key.pkcs1.der");
    assert!(matches!(
        PrivateKeyDer::try_from(RSA_2048_KEY_PKCS1).unwrap(),
        PrivateKeyDer::Pkcs1(_)
    ));

    const NIST_P256_KEY_PKCS8: &[u8] = include_bytes!("../tests/keys/nistp256key.pkcs8.der");
    const NIST_P384_KEY_PKCS8: &[u8] = include_bytes!("../tests/keys/nistp384key.pkcs8.der");
    const NIST_P521_KEY_PKCS8: &[u8] = include_bytes!("../tests/keys/nistp521key.pkcs8.der");
    const RSA_2048_KEY_PKCS8: &[u8] = include_bytes!("../tests/keys/rsa2048key.pkcs8.der");
    const RSA_4096_KEY: &[u8] = include_bytes!("../tests/keys/rsa4096key.pkcs8.der");
    const ED25519_KEY: &[u8] = include_bytes!("../tests/keys/edd25519_v2.der");
    const PKCS8_KEYS: &[&[u8]] = &[
        NIST_P256_KEY_PKCS8,
        NIST_P384_KEY_PKCS8,
        NIST_P521_KEY_PKCS8,
        RSA_2048_KEY_PKCS8,
        RSA_4096_KEY,
        ED25519_KEY,
    ];

    for &bytes in PKCS8_KEYS {
        assert!(matches!(
            PrivateKeyDer::try_from(bytes).unwrap(),
            PrivateKeyDer::Pkcs8(_)
        ));
    }
}
