use digest::dev::{feed_rand_16mib, fixed_reset_test};
use digest::new_test;
use hex_literal::hex;
use sha2::{Digest, Sha224, Sha256, Sha384, Sha512, Sha512_224, Sha512_256};

new_test!(sha224_main, "sha224", Sha224, fixed_reset_test);
new_test!(sha256_main, "sha256", Sha256, fixed_reset_test);
new_test!(sha512_224_main, "sha512_224", Sha512_224, fixed_reset_test);
new_test!(sha512_256_main, "sha512_256", Sha512_256, fixed_reset_test);
new_test!(sha384_main, "sha384", Sha384, fixed_reset_test);
new_test!(sha512_main, "sha512", Sha512, fixed_reset_test);

#[test]
fn sha256_rand() {
    let mut h = Sha256::new();
    feed_rand_16mib(&mut h);
    assert_eq!(
        h.finalize()[..],
        hex!("45f51fead87328fe837a86f4f1ac0eb15116ab1473adc0423ef86c62eb2320c7")[..]
    );
}

#[test]
#[rustfmt::skip]
fn sha512_rand() {
    let mut h = Sha512::new();
    feed_rand_16mib(&mut h);
    assert_eq!(
        h.finalize()[..],
        hex!("
            9084d75a7c0721541d737b6171eb465dc9ba08a119a182a8508484aa27a176cd
            e7c2103b108393eb024493ced4aac56be6f57222cac41b801f11494886264997
        ")[..]
    );
}
