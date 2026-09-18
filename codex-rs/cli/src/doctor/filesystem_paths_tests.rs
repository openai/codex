use std::collections::BTreeSet;
use std::time::Duration;
use std::time::Instant;

use codex_config::LoaderOverrides;
use codex_core::config::ConfigBuilder;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tokio::process::Command;

use super::ProbeResult;
use super::literal_paths;
use super::probe;
use super::source;

#[test]
fn deduplicates_literal_paths_without_expanding_globs_or_special_paths() {
    let temp = tempfile::tempdir().unwrap();
    let path = PathUri::from_host_native_path(temp.path()).unwrap();
    let mut policy = FileSystemSandboxPolicy {
        entries: vec![
            FileSystemSandboxEntry {
                path: path.clone().into(),
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: path.clone().into(),
                access: FileSystemAccessMode::Write,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: PathUri::from_host_native_path(temp.path().join("private-policy-path"))
                    .unwrap()
                    .into(),
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::GlobPattern {
                    pattern: format!("{}/**/*.env", temp.path().display()),
                },
                access: FileSystemAccessMode::Deny,
                missing_path_behavior: None,
            },
            FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
                missing_path_behavior: None,
            },
        ],
        ..Default::default()
    };
    // Equal-specificity denials, denied ancestors, and deny globs all override grants.
    for suffix in [
        "private-policy-path",
        "private-policy-path/child",
        "secret.env",
    ] {
        policy.entries.push(FileSystemSandboxEntry {
            path: PathUri::from_host_native_path(temp.path().join(suffix))
                .unwrap()
                .into(),
            access: FileSystemAccessMode::Read,
            missing_path_behavior: None,
        });
    }
    let context = FileSystemSandboxPolicyContext {
        cwd: &path,
        workspace_roots: std::slice::from_ref(&path),
        user_home_dir: None,
        temporary_directories: Some(&[]),
    };
    assert_eq!(
        literal_paths(&policy, &context),
        vec![(
            path,
            BTreeSet::from([FileSystemAccessMode::Read, FileSystemAccessMode::Write])
        )]
    );
}

#[tokio::test]
async fn attributes_paths_inherited_through_managed_profiles_to_their_config_layer() {
    let home = tempfile::tempdir().unwrap();
    let configured = home.path().join("unavailable");
    let config_file = home.path().join("config.toml");
    let base = toml::Value::String(configured.display().to_string());
    std::fs::write(
        &config_file,
        format!(
            r#"
default_permissions = "diagnostic"
[permissions.diagnostic]
extends = "middle"
[permissions.diagnostic.filesystem.{base}]
"." = "read"
[permissions.base]
extends = ":workspace"
[permissions.base.filesystem.{base}]
"." = "write"
".git" = "write"
"subtree/**" = "read"
"overridden" = "read"
"#
        ),
    )
    .unwrap();
    let requirements_file = home.path().join("requirements.toml");
    std::fs::write(
        &requirements_file,
        format!(
            r#"
[permissions.middle]
extends = "base"
[permissions.middle.filesystem.{base}]
".git/**" = "deny"
"overridden" = "write"
"#
        ),
    )
    .unwrap();
    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.system_requirements_path = Some(requirements_file);
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides)
        .build()
        .await
        .unwrap();
    let path = PathUri::from_host_native_path(configured.join(".git")).unwrap();
    assert_eq!(
        source(&config, &path),
        format!(
            "permissions.base.filesystem.{}..git in user ({})",
            configured.display(),
            config_file.display()
        )
    );
    let subtree = PathUri::from_host_native_path(configured.join("subtree")).unwrap();
    assert_eq!(
        source(&config, &subtree),
        format!(
            "permissions.base.filesystem.{}.subtree/** in user ({})",
            configured.display(),
            config_file.display()
        )
    );
    let overridden = PathUri::from_host_native_path(configured.join("overridden")).unwrap();
    assert_eq!(
        source(&config, &overridden),
        "effective filesystem policy (entry provenance unavailable)"
    );
    let base_path = PathUri::from_host_native_path(&configured).unwrap();
    assert_eq!(
        source(&config, &base_path),
        format!(
            "permissions.diagnostic.filesystem.{}.. in user ({})",
            configured.display(),
            config_file.display()
        )
    );
    let mut check = super::check(&config).await;
    for detail in &mut check.details {
        *detail = detail
            .replace(&home.path().display().to_string(), "FIXTURE")
            .replace('\\', "/");
    }
    let report = super::super::DoctorReport {
        schema_version: 1,
        generated_at: "2026-01-01T00:00:00Z".to_string(),
        overall_status: check.status,
        codex_version: "test".to_string(),
        checks: vec![check],
    };
    insta::assert_snapshot!(
        "doctor_read_restricted_filesystem_paths",
        super::super::render_human_report(
            &report,
            super::super::HumanOutputOptions {
                show_details: true,
                show_all: false,
                ascii: true,
                color_enabled: false,
            },
        )
    );
}

#[tokio::test]
async fn returns_when_a_probe_process_does_not_exit() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "--ignored",
        "--exact",
        "doctor::filesystem_paths::tests::blocked_probe_fixture",
    ]);
    let started = Instant::now();
    assert_eq!(
        probe(&mut command, Duration::from_millis(/*millis*/ 150)).await,
        ProbeResult::TimedOut
    );
    assert!(started.elapsed() < Duration::from_secs(/*secs*/ 5));
}

#[test]
#[ignore = "subprocess fixture for the bounded filesystem probe test"]
fn blocked_probe_fixture() {
    std::thread::sleep(Duration::from_secs(/*secs*/ 30));
}

#[tokio::test]
async fn attributes_home_relative_subtree_paths_to_their_config_layer() {
    let home = tempfile::tempdir().unwrap();
    let config_file = home.path().join("config.toml");
    std::fs::write(
        &config_file,
        r#"
default_permissions = "diagnostic"
[permissions.diagnostic]
extends = ":read-only"
[permissions.diagnostic.filesystem]
"~/doctor-provenance/**" = "read"
"#,
    )
    .unwrap();
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .unwrap();
    let user_home = codex_utils_absolute_path::AbsolutePathBufGuard::home_directory().unwrap();
    let path = PathUri::from_host_native_path(user_home.join("doctor-provenance")).unwrap();
    assert_eq!(
        source(&config, &path),
        format!(
            "permissions.diagnostic.filesystem.~/doctor-provenance/** in user ({})",
            config_file.display()
        )
    );
}

#[tokio::test]
async fn excludes_scoped_origins_replaced_by_a_scalar_grant() {
    let home = tempfile::tempdir().unwrap();
    let configured = home.path().join("cache.v1");
    let base = toml::Value::String(configured.display().to_string());
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"
default_permissions = "diagnostic"
[permissions.diagnostic]
extends = ":read-only"
[permissions.diagnostic.filesystem.{base}]
"." = "read"
"#
        ),
    )
    .unwrap();
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .cli_overrides(vec![(
            "permissions.diagnostic.filesystem".to_string(),
            format!("{{ {base} = \"write\" }}").parse().unwrap(),
        )])
        .build()
        .await
        .unwrap();
    let path = PathUri::from_host_native_path(&configured).unwrap();
    assert_eq!(
        source(&config, &path),
        format!(
            "permissions.diagnostic.filesystem.{} in session-flags",
            configured.display()
        )
    );
}
