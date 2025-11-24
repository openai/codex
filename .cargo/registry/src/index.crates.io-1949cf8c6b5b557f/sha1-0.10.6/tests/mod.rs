use digest::dev::{feed_rand_16mib, fixed_reset_test};
use hex_literal::hex;
use sha1::{Digest, Sha1};

digest::new_test!(sha1_main, "sha1", Sha1, fixed_reset_test);

#[test]
fn sha1_rand() {
    let mut h = Sha1::new();
    feed_rand_16mib(&mut h);
    assert_eq!(
        h.finalize()[..],
        hex!("7e565a25a8b123e9881addbcedcd927b23377a78")[..]
    );
}
