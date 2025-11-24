#![cfg(feature = "serde")]

use debugid::{CodeId, DebugId};
use uuid::Uuid;

#[test]
fn test_deserialize_debugid() {
    assert_eq!(
        DebugId::from_parts(
            Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
            0,
        ),
        serde_json::from_str("\"dfb8e43a-f242-3d73-a453-aeb6a777ef75\"").unwrap(),
    );
}

#[test]
fn test_serialize_debugid() {
    let id = DebugId::from_parts(
        Uuid::parse_str("dfb8e43a-f242-3d73-a453-aeb6a777ef75").unwrap(),
        0,
    );

    assert_eq!(
        "\"dfb8e43a-f242-3d73-a453-aeb6a777ef75\"",
        serde_json::to_string(&id).unwrap(),
    );
}

#[test]
fn test_deserialize_codeid() {
    assert_eq!(
        CodeId::new("dfb8e43af2423d73a453aeb6a777ef75".into()),
        serde_json::from_str("\"dfb8e43af2423d73a453aeb6a777ef75\"").unwrap(),
    );
}

#[test]
fn test_serialize_codeid() {
    assert_eq!(
        "\"dfb8e43af2423d73a453aeb6a777ef75\"",
        serde_json::to_string(&CodeId::new("dfb8e43af2423d73a453aeb6a777ef75".into())).unwrap(),
    );
}
