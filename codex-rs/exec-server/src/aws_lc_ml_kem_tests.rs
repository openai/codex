use clatter::bytearray::ByteArray;
use clatter::traits::Kem;
use pretty_assertions::assert_eq;

use super::AwsLcMlKem768;

#[test]
fn kem_roundtrip() {
    let keypair = AwsLcMlKem768::genkey().expect("generate keypair");
    let mut rng = clatter::crypto::rng::DefaultRng;
    let (ciphertext, encapsulated_secret) =
        AwsLcMlKem768::encapsulate(keypair.public.as_slice(), &mut rng).expect("encapsulate");
    let decapsulated_secret =
        AwsLcMlKem768::decapsulate(ciphertext.as_slice(), keypair.secret.as_slice())
            .expect("decapsulate");

    assert_eq!(
        encapsulated_secret.as_slice(),
        decapsulated_secret.as_slice()
    );
}

#[test]
fn decapsulate_rejects_wrong_ciphertext_length() {
    let keypair = AwsLcMlKem768::genkey().expect("generate keypair");

    let error = AwsLcMlKem768::decapsulate(&[], keypair.secret.as_slice())
        .expect_err("empty ciphertext should be rejected");

    assert!(matches!(error, clatter::error::KemError::Input));
}
