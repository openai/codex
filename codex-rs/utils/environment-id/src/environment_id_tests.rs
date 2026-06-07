use super::*;
use pretty_assertions::assert_eq;

#[test]
fn accepts_opaque_values_up_to_the_shared_limit() {
    for id in [
        "dev_box-1",
        "local",
        "none",
        "dev.box",
        "日本語/environment",
        "a?b#c%d",
    ] {
        assert_eq!(EnvironmentId::new(id), Ok(EnvironmentId(id.to_string())));
    }
    assert_eq!(
        EnvironmentId::new("x".repeat(MAX_ENVIRONMENT_ID_LEN)),
        Ok(EnvironmentId("x".repeat(MAX_ENVIRONMENT_ID_LEN)))
    );
}

#[test]
fn rejects_values_that_cannot_cross_codex_boundaries() {
    assert_eq!(EnvironmentId::new(""), Err(EnvironmentIdError::Empty));
    for id in [".", ".."] {
        assert_eq!(
            EnvironmentId::new(id),
            Err(EnvironmentIdError::DotSegment(id.to_string()))
        );
    }
    assert_eq!(
        EnvironmentId::new("x".repeat(MAX_ENVIRONMENT_ID_LEN + 1)),
        Err(EnvironmentIdError::TooLong {
            length: MAX_ENVIRONMENT_ID_LEN + 1,
            max_length: MAX_ENVIRONMENT_ID_LEN,
        })
    );
}

#[test]
fn serde_uses_the_validated_string_representation() {
    let id = EnvironmentId::new("dev/environment").expect("valid environment id");
    let json = serde_json::to_string(&id).expect("environment id should serialize");
    let deserialized =
        serde_json::from_str::<EnvironmentId>(&json).expect("environment id should deserialize");

    assert_eq!(json, r#""dev/environment""#);
    assert_eq!(deserialized, id);
    assert!(serde_json::from_str::<EnvironmentId>(r#""..""#).is_err());
}
