use pretty_assertions::assert_eq;

use super::InitiatorHandshake;
use super::MAX_MESSAGE_LEN;
use super::NOISE_CHANNEL_SUITE;
use super::NoiseChannelError;
use super::NoiseChannelIdentity;
use super::NoiseChannelPublicKey;

#[test]
fn public_key_validation_rejects_unknown_suite() {
    let key = NoiseChannelIdentity::generate()
        .expect("generate identity")
        .public_key();
    let json = serde_json::to_value(key).expect("serialize key");
    let mut object = json.as_object().expect("key object").clone();
    object.insert("suite".to_string(), serde_json::json!("unknown"));
    let key: NoiseChannelPublicKey =
        serde_json::from_value(serde_json::Value::Object(object)).expect("deserialize key");

    let initiator = NoiseChannelIdentity::generate().expect("generate initiator identity");
    assert!(InitiatorHandshake::start(&initiator, &key, b"prologue", b"").is_err());
}

#[test]
fn public_key_serializes_with_expected_suite() {
    let key = NoiseChannelIdentity::generate()
        .expect("generate identity")
        .public_key();

    let json = serde_json::to_value(key).expect("serialize key");

    assert_eq!(json["suite"], NOISE_CHANNEL_SUITE);
}

#[test]
fn initiator_rejects_oversized_handshake_payload() {
    let initiator = NoiseChannelIdentity::generate().expect("generate initiator identity");
    let responder = NoiseChannelIdentity::generate().expect("generate responder identity");
    let payload = vec![0; MAX_MESSAGE_LEN];

    let result =
        InitiatorHandshake::start(&initiator, &responder.public_key(), b"prologue", &payload);

    assert!(matches!(
        result,
        Err(NoiseChannelError::InvalidMessage(
            "handshake payload is too large"
        ))
    ));
}
