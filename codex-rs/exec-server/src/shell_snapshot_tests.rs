use std::collections::HashMap;

use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
use pretty_assertions::assert_eq;

use super::parse_snapshot;
use crate::protocol::ExecEnvPolicy;

#[test]
fn snapshot_filters_profile_exports_after_capture() {
    let policy = ExecEnvPolicy {
        inherit: ShellEnvironmentPolicyInherit::All,
        ignore_default_excludes: false,
        exclude: vec!["PROFILE_DENIED".to_string()],
        r#set: HashMap::from([("PROFILE_ALLOWED".to_string(), "override".to_string())]),
        include_only: vec!["PROFILE_*".to_string()],
    };
    let snapshot = parse_snapshot(
        b"profile noise\n# Snapshot file\nfunction profile_helper() { :; }\n\0PROFILE_ALLOWED=profile\0PROFILE_DENIED=denied\0PROFILE_SECRET=secret\0PWD=/tmp\0",
        Some(&policy),
    )
    .expect("snapshot should parse");

    assert_eq!(
        snapshot.environment,
        HashMap::from([("PROFILE_ALLOWED".to_string(), "override".to_string())])
    );
    assert_eq!(
        snapshot.state,
        "# Snapshot file\nfunction profile_helper() { :; }\n"
    );
}

#[test]
fn snapshot_preserves_profile_exports_with_restrictive_inheritance() {
    for inherit in [
        ShellEnvironmentPolicyInherit::None,
        ShellEnvironmentPolicyInherit::Core,
    ] {
        let policy = ExecEnvPolicy {
            inherit,
            ignore_default_excludes: false,
            exclude: vec!["PROFILE_DENIED".to_string()],
            r#set: HashMap::new(),
            include_only: Vec::new(),
        };
        let snapshot = parse_snapshot(
            b"# Snapshot file\n\0PROFILE_ALLOWED=profile\0SDKROOT=/sdk\0PROFILE_SECRET=secret\0PROFILE_DENIED=denied\0",
            Some(&policy),
        )
        .expect("snapshot should parse");

        assert_eq!(
            snapshot.environment,
            HashMap::from([
                ("PROFILE_ALLOWED".to_string(), "profile".to_string()),
                ("SDKROOT".to_string(), "/sdk".to_string()),
            ])
        );
    }
}

#[test]
fn snapshot_caches_only_unmanaged_proxy_state() {
    for (exports, expected) in [
        (
            "PROFILE_ALLOWED=profile\0HTTP_PROXY=http://127.0.0.1:4321\0CODEX_NETWORK_PROXY_ACTIVE=1\0CODEX_NETWORK_PROXY_CREDENTIAL_BROKER_ACTIVE=1\0",
            HashMap::from([("PROFILE_ALLOWED".to_string(), "profile".to_string())]),
        ),
        (
            "PROFILE_ALLOWED=profile\0HTTP_PROXY=http://user-proxy.example\0",
            HashMap::from([
                ("PROFILE_ALLOWED".to_string(), "profile".to_string()),
                (
                    "HTTP_PROXY".to_string(),
                    "http://user-proxy.example".to_string(),
                ),
            ]),
        ),
    ] {
        let output = format!("# Snapshot file\n\0{exports}");
        let snapshot =
            parse_snapshot(output.as_bytes(), /*env_policy*/ None).expect("snapshot should parse");

        assert_eq!(snapshot.environment, expected);
    }
}
