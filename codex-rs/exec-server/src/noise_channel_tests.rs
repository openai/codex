use pretty_assertions::assert_eq;

use super::NOISE_CHANNEL_SUITE;
use super::NoiseChannelIdentity;

#[test]
fn public_key_serializes_with_expected_suite() {
    let key = NoiseChannelIdentity::generate()
        .expect("generate identity")
        .public_key();

    let json = serde_json::to_value(key).expect("serialize key");

    assert_eq!(json["suite"], NOISE_CHANNEL_SUITE);
}
