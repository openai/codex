//! Verify bounded transport, Unicode roundtrips, and child-environment cleanup.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use pretty_assertions::assert_eq;

use crate::MxcCommand;

use super::CHUNK_BYTES;
use super::COUNT;
use super::LENGTH;
use super::MAX_BYTES;
use super::PREFIX;
use super::decode;
use super::encode;

fn command(args: Vec<String>) -> MxcCommand {
    MxcCommand {
        permissions: PermissionProfile::read_only(),
        sandbox_policy_cwd: PathBuf::from("workspace"),
        managed_network: None,
        command: args,
    }
}

#[test]
fn large_unicode_launch_roundtrips_and_never_reaches_child_environment() -> Result<()> {
    let args = vec![
        "codex-windows-mxc".to_owned(),
        "🧊".repeat(20_000),
        "a\"\\b".to_owned(),
    ];
    let expected_env = HashMap::from([("CUSTOM".to_owned(), "value".to_owned())]);
    let mut env = expected_env.clone();
    env.insert("codex_mxc_launch_999".to_owned(), "spoofed".to_owned());
    let request = command(args);
    encode(&request, &mut env)?;
    assert!(!env.contains_key("codex_mxc_launch_999"));
    assert!(
        env.values()
            .all(|value| value.encode_utf16().count() < 8192)
    );
    assert_eq!(
        serde_json::to_value(decode(&mut env)?)?,
        serde_json::to_value(request)?
    );
    assert_eq!(env, expected_env);
    Ok(())
}

#[test]
fn malformed_transport_is_rejected_and_removed() -> Result<()> {
    let expected_env = HashMap::from([("CUSTOM".to_owned(), "value".to_owned())]);
    let mut valid = expected_env.clone();
    encode(&command(vec!["program.exe".to_owned()]), &mut valid)?;
    for (key, value) in [
        (COUNT.to_owned(), "257".to_owned()),
        (LENGTH.to_owned(), "0".to_owned()),
        (format!("{PREFIX}0"), "x".repeat(CHUNK_BYTES + 1)),
        (COUNT.to_ascii_lowercase(), "1".to_owned()),
    ] {
        let mut env = valid.clone();
        env.insert(key, value);
        assert!(decode(&mut env).is_err());
        assert_eq!(env, expected_env);
    }
    valid.remove(&format!("{PREFIX}0"));
    assert!(decode(&mut valid).is_err());
    assert_eq!(valid, expected_env);
    Ok(())
}

#[test]
fn oversized_payload_fails_before_modifying_launcher_environment() {
    let expected = HashMap::from([("CUSTOM".to_owned(), "value".to_owned())]);
    let mut env = expected.clone();
    assert!(encode(&command(vec!["x".repeat(MAX_BYTES)]), &mut env).is_err());
    assert_eq!(env, expected);
}
