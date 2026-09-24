//! Verify bounded transport, Unicode roundtrips, and child-environment cleanup.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use codex_windows_sandbox::environment_transport;
use pretty_assertions::assert_eq;

use crate::MxcCommand;

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
    environment_transport::encode("stale", &mut env)?;
    let request = command(args);
    encode(&request, &mut env)?;
    assert!(
        env.values()
            .all(|value| value.encode_utf16().count() < 32_767)
    );
    assert_eq!(
        serde_json::to_value(decode(&mut env)?)?,
        serde_json::to_value(request)?
    );
    assert_eq!(env, expected_env);
    Ok(())
}
