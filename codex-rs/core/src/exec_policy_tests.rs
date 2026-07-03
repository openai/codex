use super::*;
use crate::config::Config;
use crate::config::ConfigBuilder;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_config::LoaderOverrides;
use codex_config::RequirementSource;
use codex_config::RequirementsExecPolicy;
use codex_config::Sourced;
use codex_config::config_toml::ConfigToml;
use codex_config::config_toml::ProjectConfig;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tempfile::tempdir;
use toml::Value as TomlValue;

#[cfg(windows)]
#[path = "exec_policy_windows_tests.rs"]
mod windows_tests;

#[cfg(windows)]
#[path = "exec_policy_powershell_tests.rs"]
mod powershell_tests;

fn config_stack_for_dot_codex_folder(dot_codex_folder: &Path) -> ConfigLayerStack {
    let dot_codex_folder =
        AbsolutePathBuf::from_absolute_path(dot_codex_folder).expect("absolute dot_codex_folder");
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::Project { dot_codex_folder },
        TomlValue::Table(Default::default()),
    );
    ConfigLayerStack::new(
        vec![layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("ConfigLayerStack")
}

fn host_absolute_path(segments: &[&str]) -> String {
    let mut path = if cfg!(windows) {
        PathBuf::from(r"C:\")
    } else {
        PathBuf::from("/")
    };
    for segment in segments {
        path.push(segment);
    }
    path.to_string_lossy().into_owned()
}

fn host_program_path(name: &str) -> String {
    let executable_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    host_absolute_path(&["usr", "bin", &executable_name])
}

fn posix_script_program_path(name: &str) -> String {
    host_program_path(name).replace('\\', "/")
}

fn starlark_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn prefix_rule_for(pattern: &[String], decision: &str) -> String {
    let pattern = pattern
        .iter()
        .map(|token| format!("\"{}\"", starlark_string(token)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("prefix_rule(pattern=[{pattern}], decision=\"{decision}\")")
}

async fn requirement_with_provenance(
    policy_src: Option<&str>,
    command: &[String],
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    sandbox_permissions: SandboxPermissions,
    provenance: ShellApprovalProvenance,
) -> ExecApprovalRequirement {
    let permission_expansion_was_requested = sandbox_permissions.requests_sandbox_override();
    ExecPolicyManager::new(policy_from_src(policy_src))
        .create_exec_approval_requirement_for_command_with_provenance(
            ExecApprovalRequest {
                command,
                approval_policy,
                permission_profile,
                windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
                sandbox_permissions,
                prefix_rule: None,
            },
            provenance,
            permission_expansion_was_requested,
        )
        .await
}

async fn write_project_trust_config(
    codex_home: &Path,
    trusted_projects: &[(&Path, TrustLevel)],
) -> std::io::Result<()> {
    tokio::fs::write(
        codex_home.join(codex_config::CONFIG_TOML_FILE),
        toml::to_string(&ConfigToml {
            projects: Some(
                trusted_projects
                    .iter()
                    .map(|(project, trust_level)| {
                        (
                            project.to_string_lossy().to_string(),
                            ProjectConfig {
                                trust_level: Some(*trust_level),
                            },
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>(),
            ),
            ..Default::default()
        })
        .expect("serialize config"),
    )
    .await
}

async fn test_config() -> (TempDir, Config) {
    let home = TempDir::new().expect("create temp dir");
    let config = ConfigBuilder::without_managed_config_for_tests()
        .codex_home(home.path().to_path_buf())
        .build()
        .await
        .expect("load default test config");
    (home, config)
}

#[tokio::test]
async fn child_uses_parent_exec_policy_when_layer_stack_matches() {
    let (_home, parent_config) = test_config().await;
    let child_config = parent_config.clone();

    assert!(child_uses_parent_exec_policy(&parent_config, &child_config));
}

#[tokio::test]
async fn child_uses_parent_exec_policy_when_non_exec_policy_layers_differ() {
    let (_home, parent_config) = test_config().await;
    let mut child_config = parent_config.clone();
    let mut layers: Vec<_> = child_config
        .config_layer_stack
        .get_layers(
            ConfigLayerStackOrdering::LowestPrecedenceFirst,
            /*include_disabled*/ true,
        )
        .into_iter()
        .cloned()
        .collect();
    layers.push(ConfigLayerEntry::new(
        ConfigLayerSource::SessionFlags,
        TomlValue::Table(Default::default()),
    ));
    child_config.config_layer_stack = ConfigLayerStack::new(
        layers,
        child_config.config_layer_stack.requirements().clone(),
        child_config.config_layer_stack.requirements_toml().clone(),
    )
    .expect("config layer stack");

    assert!(child_uses_parent_exec_policy(&parent_config, &child_config));
}

#[tokio::test]
async fn child_does_not_use_parent_exec_policy_when_ignore_rules_differs() {
    let (_home, parent_config) = test_config().await;
    let mut child_config = parent_config.clone();
    child_config.config_layer_stack = child_config
        .config_layer_stack
        .with_user_and_project_exec_policy_rules_ignored(
            /*ignore_user_and_project_exec_policy_rules*/ true,
        );

    assert!(!child_uses_parent_exec_policy(
        &parent_config,
        &child_config
    ));
}

#[tokio::test]
async fn child_does_not_use_parent_exec_policy_when_requirements_exec_policy_differs() {
    let (_home, parent_config) = test_config().await;
    let mut child_config = parent_config.clone();
    let mut requirements = ConfigRequirements {
        exec_policy: child_config
            .config_layer_stack
            .requirements()
            .exec_policy
            .clone(),
        ..ConfigRequirements::default()
    };
    let mut policy = Policy::empty();
    policy
        .add_prefix_rule(&["rm".to_string()], Decision::Forbidden)
        .expect("add prefix rule");
    requirements.exec_policy = Some(Sourced::new(
        RequirementsExecPolicy::new(policy),
        RequirementSource::Unknown,
    ));
    child_config.config_layer_stack = ConfigLayerStack::new(
        child_config
            .config_layer_stack
            .get_layers(
                ConfigLayerStackOrdering::LowestPrecedenceFirst,
                /*include_disabled*/ true,
            )
            .into_iter()
            .cloned()
            .collect(),
        requirements,
        child_config.config_layer_stack.requirements_toml().clone(),
    )
    .expect("config layer stack");

    assert!(!child_uses_parent_exec_policy(
        &parent_config,
        &child_config
    ));
}

#[tokio::test]
async fn returns_empty_policy_when_no_policy_files_exist() {
    let temp_dir = tempdir().expect("create temp dir");
    let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());

    let manager = ExecPolicyManager::load(&config_stack)
        .await
        .expect("manager result");
    let policy = manager.current();

    let commands = [vec!["rm".to_string()]];
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command: vec!["rm".to_string()],
                decision: Decision::Allow
            }],
        },
        policy.check_multiple(commands.iter(), &|_| Decision::Allow)
    );
    assert!(!temp_dir.path().join(RULES_DIR_NAME).exists());
}

#[tokio::test]
async fn rules_path_file_returns_read_dir_error() {
    let temp_dir = tempdir().expect("create temp dir");
    let rules_path = temp_dir.path().join(RULES_DIR_NAME);
    fs::write(&rules_path, "rules should be a directory").expect("write malformed rules path");
    let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());

    let err = load_exec_policy(&config_stack)
        .await
        .expect_err("rules file should fail policy loading");

    assert!(
        matches!(
            err,
            ExecPolicyError::ReadDir { ref dir, .. } if dir == &rules_path
        ),
        "expected malformed rules path to surface as ReadDir, got {err:?}"
    );
}

#[tokio::test]
async fn collect_policy_files_returns_empty_when_dir_missing() {
    let temp_dir = tempdir().expect("create temp dir");

    let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
    let files = collect_policy_files(&policy_dir)
        .await
        .expect("collect policy files");

    assert!(files.is_empty());
}

#[tokio::test]
async fn format_exec_policy_error_with_source_renders_range() {
    let temp_dir = tempdir().expect("create temp dir");
    let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());
    let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
    fs::create_dir_all(&policy_dir).expect("create policy dir");
    let broken_path = policy_dir.join("broken.rules");
    fs::write(
        &broken_path,
        r#"prefix_rule(
    pattern = ["tmux capture-pane"],
    decision = "allow",
    match = ["tmux capture-pane -p"],
)"#,
    )
    .expect("write broken policy file");

    let err = load_exec_policy(&config_stack)
        .await
        .expect_err("expected parse error");
    let rendered = format_exec_policy_error_with_source(&err);

    assert!(rendered.contains("broken.rules:1:"));
    assert!(rendered.contains("on or around line 1"));
}

#[test]
fn parse_starlark_line_from_message_extracts_path_and_line() {
    let parsed = parse_starlark_line_from_message(
        "/tmp/default.rules:143:1: starlark error: error: Parse error: unexpected new line",
    )
    .expect("parse should succeed");

    assert_eq!(parsed.0, PathBuf::from("/tmp/default.rules"));
    assert_eq!(parsed.1, 143);
}

#[test]
fn parse_starlark_line_from_message_rejects_zero_line() {
    let parsed = parse_starlark_line_from_message(
        "/tmp/default.rules:0:1: starlark error: error: Parse error: unexpected new line",
    );
    assert_eq!(parsed, None);
}

#[tokio::test]
async fn loads_policies_from_policy_subdirectory() {
    let temp_dir = tempdir().expect("create temp dir");
    let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());
    let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
    fs::create_dir_all(&policy_dir).expect("create policy dir");
    fs::write(
        policy_dir.join("deny.rules"),
        r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
    )
    .expect("write policy file");

    let policy = load_exec_policy(&config_stack)
        .await
        .expect("policy result");
    let command = [vec!["rm".to_string()]];
    assert_eq!(
        Evaluation {
            decision: Decision::Forbidden,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: vec!["rm".to_string()],
                decision: Decision::Forbidden,
                resolved_program: None,
                justification: None,
            }],
        },
        policy.check_multiple(command.iter(), &|_| Decision::Allow)
    );
}

#[tokio::test]
async fn merges_requirements_exec_policy_network_rules() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;

    let mut requirements_exec_policy = Policy::empty();
    requirements_exec_policy.add_network_rule(
        "blocked.example.com",
        codex_execpolicy::NetworkRuleProtocol::Https,
        Decision::Forbidden,
        /*justification*/ None,
    )?;

    let requirements = ConfigRequirements {
        exec_policy: Some(codex_config::Sourced::new(
            codex_config::RequirementsExecPolicy::new(requirements_exec_policy),
            codex_config::RequirementSource::Unknown,
        )),
        ..ConfigRequirements::default()
    };
    let dot_codex_folder = AbsolutePathBuf::from_absolute_path(temp_dir.path())?;
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::Project { dot_codex_folder },
        TomlValue::Table(Default::default()),
    );
    let config_stack =
        ConfigLayerStack::new(vec![layer], requirements, ConfigRequirementsToml::default())?;

    let policy = load_exec_policy(&config_stack).await?;
    let (allowed, denied) = policy.compiled_network_domains();

    assert!(allowed.is_empty());
    assert_eq!(denied, vec!["blocked.example.com".to_string()]);
    Ok(())
}

#[tokio::test]
async fn preserves_host_executables_when_requirements_overlay_is_present() -> anyhow::Result<()> {
    let temp_dir = tempdir()?;
    let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
    fs::create_dir_all(&policy_dir)?;
    let git_path = host_absolute_path(&["usr", "bin", "git"]);
    let git_path_literal = starlark_string(&git_path);
    fs::write(
        policy_dir.join("host.rules"),
        format!(
            r#"
host_executable(name = "git", paths = ["{git_path_literal}"])
"#
        ),
    )?;

    let mut requirements_exec_policy = Policy::empty();
    requirements_exec_policy.add_network_rule(
        "blocked.example.com",
        codex_execpolicy::NetworkRuleProtocol::Https,
        Decision::Forbidden,
        /*justification*/ None,
    )?;

    let requirements = ConfigRequirements {
        exec_policy: Some(codex_config::Sourced::new(
            codex_config::RequirementsExecPolicy::new(requirements_exec_policy),
            codex_config::RequirementSource::Unknown,
        )),
        ..ConfigRequirements::default()
    };
    let dot_codex_folder = AbsolutePathBuf::from_absolute_path(temp_dir.path())?;
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::Project { dot_codex_folder },
        TomlValue::Table(Default::default()),
    );
    let config_stack =
        ConfigLayerStack::new(vec![layer], requirements, ConfigRequirementsToml::default())?;

    let policy = load_exec_policy(&config_stack).await?;

    assert_eq!(
        policy
            .host_executables()
            .get("git")
            .expect("missing git host executable")
            .as_ref(),
        [AbsolutePathBuf::try_from(git_path)?]
    );
    Ok(())
}

#[tokio::test]
async fn ignores_policies_outside_policy_dir() {
    let temp_dir = tempdir().expect("create temp dir");
    let config_stack = config_stack_for_dot_codex_folder(temp_dir.path());
    fs::write(
        temp_dir.path().join("root.rules"),
        r#"prefix_rule(pattern=["ls"], decision="prompt")"#,
    )
    .expect("write policy file");

    let policy = load_exec_policy(&config_stack)
        .await
        .expect("policy result");
    let command = [vec!["ls".to_string()]];
    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command: vec!["ls".to_string()],
                decision: Decision::Allow
            }],
        },
        policy.check_multiple(command.iter(), &|_| Decision::Allow)
    );
}

#[tokio::test]
async fn ignores_policy_files_when_config_stack_disables_exec_policy_rules() {
    let temp_dir = tempdir().expect("create temp dir");
    let policy_dir = temp_dir.path().join(RULES_DIR_NAME);
    fs::create_dir_all(&policy_dir).expect("create policy dir");
    fs::write(
        policy_dir.join("allow.rules"),
        r#"prefix_rule(pattern=["curl"], decision="allow")"#,
    )
    .expect("write policy file");
    let config_stack = config_stack_for_dot_codex_folder(temp_dir.path())
        .with_user_and_project_exec_policy_rules_ignored(
            /*ignore_user_and_project_exec_policy_rules*/ true,
        );

    let policy = load_exec_policy(&config_stack)
        .await
        .expect("policy result");

    assert_eq!(
        policy
            .check_multiple([vec!["curl".to_string()]].iter(), &|_| Decision::Forbidden)
            .decision,
        Decision::Forbidden,
    );
}

#[tokio::test]
async fn ignore_user_project_rules_keeps_system_policy_files() {
    let temp_dir = tempdir().expect("create temp dir");
    let config_dir = temp_dir.path().join("system");
    let policy_dir = config_dir.join(RULES_DIR_NAME);
    fs::create_dir_all(&policy_dir).expect("create policy dir");
    fs::write(
        policy_dir.join("allow.rules"),
        r#"prefix_rule(pattern=["curl"], decision="allow")"#,
    )
    .expect("write policy file");
    let config_file =
        AbsolutePathBuf::from_absolute_path(config_dir.join(codex_config::CONFIG_TOML_FILE))
            .expect("absolute config file");
    let layer = ConfigLayerEntry::new(
        ConfigLayerSource::System { file: config_file },
        TomlValue::Table(Default::default()),
    );
    let config_stack = ConfigLayerStack::new(
        vec![layer],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("ConfigLayerStack")
    .with_user_and_project_exec_policy_rules_ignored(
        /*ignore_user_and_project_exec_policy_rules*/ true,
    );

    let policy = load_exec_policy(&config_stack)
        .await
        .expect("policy result");

    assert_eq!(
        policy
            .check_multiple([vec!["curl".to_string()]].iter(), &|_| Decision::Forbidden)
            .decision,
        Decision::Allow,
    );
}

#[tokio::test]
async fn ignores_rules_from_untrusted_project_layers() -> anyhow::Result<()> {
    let project_dir = tempdir()?;
    let policy_dir = project_dir.path().join(RULES_DIR_NAME);
    fs::create_dir_all(&policy_dir)?;
    fs::write(
        policy_dir.join("untrusted.rules"),
        r#"prefix_rule(pattern=["ls"], decision="forbidden")"#,
    )?;

    let project_dot_codex_folder = AbsolutePathBuf::from_absolute_path(project_dir.path())?;
    let layers = vec![ConfigLayerEntry::new_disabled(
        ConfigLayerSource::Project {
            dot_codex_folder: project_dot_codex_folder,
        },
        TomlValue::Table(Default::default()),
        "marked untrusted",
    )];
    let config_stack = ConfigLayerStack::new(
        layers,
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )?;

    let policy = load_exec_policy(&config_stack).await?;

    assert_eq!(
        Evaluation {
            decision: Decision::Allow,
            matched_rules: vec![RuleMatch::HeuristicsRuleMatch {
                command: vec!["ls".to_string()],
                decision: Decision::Allow,
            }],
        },
        policy.check_multiple([vec!["ls".to_string()]].iter(), &|_| Decision::Allow)
    );
    Ok(())
}

#[tokio::test]
async fn loads_policies_from_multiple_config_layers() -> anyhow::Result<()> {
    let user_dir = tempdir()?;
    let project_dir = tempdir()?;

    let user_policy_dir = user_dir.path().join(RULES_DIR_NAME);
    fs::create_dir_all(&user_policy_dir)?;
    fs::write(
        user_policy_dir.join("user.rules"),
        r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
    )?;

    let project_policy_dir = project_dir.path().join(RULES_DIR_NAME);
    fs::create_dir_all(&project_policy_dir)?;
    fs::write(
        project_policy_dir.join("project.rules"),
        r#"prefix_rule(pattern=["ls"], decision="prompt")"#,
    )?;

    let user_config_toml =
        AbsolutePathBuf::from_absolute_path(user_dir.path().join("config.toml"))?;
    let project_dot_codex_folder = AbsolutePathBuf::from_absolute_path(project_dir.path())?;
    let layers = vec![
        ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: user_config_toml,
                profile: None,
            },
            TomlValue::Table(Default::default()),
        ),
        ConfigLayerEntry::new(
            ConfigLayerSource::Project {
                dot_codex_folder: project_dot_codex_folder,
            },
            TomlValue::Table(Default::default()),
        ),
    ];
    let config_stack = ConfigLayerStack::new(
        layers,
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )?;

    let policy = load_exec_policy(&config_stack).await?;

    assert_eq!(
        Evaluation {
            decision: Decision::Forbidden,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: vec!["rm".to_string()],
                decision: Decision::Forbidden,
                resolved_program: None,
                justification: None,
            }],
        },
        policy.check_multiple([vec!["rm".to_string()]].iter(), &|_| Decision::Allow)
    );
    assert_eq!(
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: vec!["ls".to_string()],
                decision: Decision::Prompt,
                resolved_program: None,
                justification: None,
            }],
        },
        policy.check_multiple([vec!["ls".to_string()]].iter(), &|_| Decision::Allow)
    );
    Ok(())
}

#[tokio::test]
async fn evaluates_bash_lc_inner_commands() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["rm"], decision="forbidden")"#.to_string()),
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "rm -rf /some/important/folder".to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Forbidden {
            reason: "`bash -lc 'rm -rf /some/important/folder'` rejected: policy forbids commands starting with `rm`".to_string(),
        },
    )
    .await;
}

#[test]
fn commands_for_exec_policy_falls_back_for_empty_shell_script() {
    let command = vec!["bash".to_string(), "-lc".to_string(), "".to_string()];

    assert_eq!(
        commands_for_exec_policy(&command),
        ExecPolicyCommands {
            commands: vec![command],
            used_complex_parsing: false,
            command_origin: ExecPolicyCommandOrigin::Generic,
        }
    );
}

#[test]
fn commands_for_exec_policy_falls_back_for_whitespace_shell_script() {
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "  \n\t  ".to_string(),
    ];

    assert_eq!(
        commands_for_exec_policy(&command),
        ExecPolicyCommands {
            commands: vec![command],
            used_complex_parsing: false,
            command_origin: ExecPolicyCommandOrigin::Generic,
        }
    );
}

#[tokio::test]
async fn ignore_user_config_keeps_user_policy_files() -> std::io::Result<()> {
    let temp = tempdir()?;
    let codex_home = temp.path().join("home_ignore_user_config");
    let rules_dir = codex_home.join(RULES_DIR_NAME);
    fs::create_dir_all(&rules_dir)?;
    fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        "model = \"from-user-config\"\ninvalid = [",
    )?;
    fs::write(
        rules_dir.join("deny-curl.rules"),
        r#"prefix_rule(pattern=["curl"], decision="forbidden")"#,
    )?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home)
        .fallback_cwd(Some(temp.path().to_path_buf()))
        .loader_overrides(LoaderOverrides {
            ignore_user_config: true,
            ..Default::default()
        })
        .build()
        .await?;

    let policy = load_exec_policy(&config.config_layer_stack)
        .await
        .map_err(std::io::Error::other)?;

    assert_eq!(
        policy
            .check_multiple([vec!["curl".to_string()]].iter(), &|_| Decision::Allow)
            .decision,
        Decision::Forbidden,
    );

    Ok(())
}

#[tokio::test]
async fn evaluates_heredoc_script_against_prefix_rules() {
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "python3 <<'PY'\nprint('hello')\nPY".to_string(),
    ];

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["python3"], decision="allow")"#.to_string()),
            command,
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn omits_auto_amendment_for_heredoc_fallback_prompts() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "python3 <<'PY'\nprint('hello')\nPY".to_string(),
            ],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn drops_requested_amendment_for_heredoc_fallback_prompts_when_it_wont_match() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "python3 <<'PY'\nprint('hello')\nPY".to_string(),
            ],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: Some(vec![
                "python3".to_string(),
                "-m".to_string(),
                "pip".to_string(),
            ]),
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn drops_requested_amendment_for_heredoc_fallback_prompts_when_it_matches() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "python3 <<'PY'\nprint('hello')\nPY".to_string(),
            ],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: Some(vec!["python3".to_string()]),
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
#[cfg(not(windows))]
async fn heredoc_with_variable_assignment_is_not_reduced_to_allowed_prefix() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["cat"], decision="allow")"#.to_string()),
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "PATH=/tmp/evil:$PATH cat <<'EOF'\nhello\nEOF".to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                "bash".to_string(),
                "-lc".to_string(),
                "PATH=/tmp/evil:$PATH cat <<'EOF'\nhello\nEOF".to_string(),
            ])),
        },
    )
    .await;
}

#[tokio::test]
async fn heredoc_redirect_without_escalation_runs_inside_sandbox() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                "zsh".to_string(),
                "-lc".to_string(),
                r#"cat <<'EOF' > /some/important/folder/test.txt
hello world
EOF"#
                    .to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::workspace_write(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                "zsh".to_string(),
                "-lc".to_string(),
                r#"cat <<'EOF' > /some/important/folder/test.txt
hello world
EOF"#
                    .to_string(),
            ])),
        },
    )
    .await;
}

#[tokio::test]
async fn heredoc_redirect_with_escalation_requires_approval() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["cat"], decision="allow")"#.to_string()),
            command: vec![
                "zsh".to_string(),
                "-lc".to_string(),
                r#"cat <<'EOF' > /some/important/folder/test.txt
hello world
EOF"#
                    .to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::workspace_write(),
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn justification_is_included_in_forbidden_exec_approval_requirement() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(
                r#"
prefix_rule(
    pattern=["rm"],
    decision="forbidden",
    justification="destructive command",
)
"#
                .to_string(),
            ),
            command: vec![
                "rm".to_string(),
                "-rf".to_string(),
                "/some/important/folder".to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Forbidden {
            reason: "`rm -rf /some/important/folder` rejected: destructive command".to_string(),
        },
    )
    .await;
}

#[tokio::test]
async fn exec_approval_requirement_prefers_execpolicy_match() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["rm"], decision="prompt")"#.to_string()),
            command: vec!["rm".to_string()],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: Some("`rm` requires approval by policy".to_string()),
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn absolute_path_exec_approval_requirement_matches_host_executable_rules() {
    let git_path = host_program_path("git");
    let git_path_literal = starlark_string(&git_path);
    let policy_src = format!(
        r#"
host_executable(name = "git", paths = ["{git_path_literal}"])
prefix_rule(pattern=["git"], decision="allow")
"#
    );
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(policy_src),
            command: vec![git_path, "status".to_string()],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn absolute_path_exec_approval_requirement_ignores_disallowed_host_executable_paths() {
    let allowed_git_path = host_program_path("git");
    let disallowed_git_path = host_absolute_path(&[
        "opt",
        "homebrew",
        "bin",
        if cfg!(windows) { "git.exe" } else { "git" },
    ]);
    let allowed_git_path_literal = starlark_string(&allowed_git_path);
    let policy_src = format!(
        r#"
host_executable(name = "git", paths = ["{allowed_git_path_literal}"])
prefix_rule(pattern=["git"], decision="prompt")
"#
    );
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(policy_src),
            command: vec![disallowed_git_path.clone(), "status".to_string()],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                disallowed_git_path,
                "status".to_string(),
            ])),
        },
    )
    .await;
}

#[tokio::test]
async fn requested_prefix_rule_can_approve_absolute_path_commands() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                host_program_path("cargo"),
                "install".to_string(),
                "cargo-insta".to_string(),
            ],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: Some(vec!["cargo".to_string(), "install".to_string()]),
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                "cargo".to_string(),
                "install".to_string(),
            ])),
        },
    )
    .await;
}

#[tokio::test]
async fn exec_approval_requirement_respects_approval_policy() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["rm"], decision="prompt")"#.to_string()),
            command: vec!["rm".to_string()],
            approval_policy: AskForApproval::Never,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Forbidden {
            reason: PROMPT_CONFLICT_REASON.to_string(),
        },
    )
    .await;
}

#[test]
fn unmatched_granular_policy_still_prompts_for_restricted_sandbox_escalation() {
    let command = vec!["madeup-cmd".to_string()];

    assert_eq!(
        Decision::Prompt,
        render_decision_for_unmatched_command(
            &command,
            UnmatchedCommandContext {
                approval_policy: AskForApproval::Granular(GranularApprovalConfig {
                    sandbox_approval: true,
                    rules: true,
                    skill_approval: true,
                    request_permissions: true,
                    mcp_elicitations: true,
                }),
                permission_profile: &PermissionProfile::read_only(),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::RequireEscalated,
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::Generic,
            },
        )
    );
}

#[test]
fn unmatched_on_request_uses_permission_profile_file_system_policy_for_escalation_prompts() {
    let command = vec!["madeup-cmd".to_string()];

    assert_eq!(
        Decision::Prompt,
        render_decision_for_unmatched_command(
            &command,
            UnmatchedCommandContext {
                approval_policy: AskForApproval::OnRequest,
                permission_profile: &PermissionProfile::read_only(),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::RequireEscalated,
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::Generic,
            },
        )
    );
}

#[test]
fn known_safe_on_request_still_prompts_for_restricted_sandbox_escalation() {
    let command = vec!["echo".to_string(), "hello".to_string()];

    assert_eq!(
        Decision::Prompt,
        render_decision_for_unmatched_command(
            &command,
            UnmatchedCommandContext {
                approval_policy: AskForApproval::OnRequest,
                permission_profile: &PermissionProfile::workspace_write(),
                windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
                sandbox_permissions: SandboxPermissions::RequireEscalated,
                used_complex_parsing: false,
                command_origin: ExecPolicyCommandOrigin::Generic,
            },
        )
    );
}

#[test]
fn known_safe_sandbox_override_is_checked_before_the_safelist() {
    let command = vec_str(&["echo", "hello"]);
    let granular = GranularApprovalConfig {
        sandbox_approval: true,
        rules: true,
        skill_approval: true,
        request_permissions: true,
        mcp_elicitations: true,
    };

    for (approval_policy, expected) in [
        (AskForApproval::OnRequest, Decision::Prompt),
        (AskForApproval::UnlessTrusted, Decision::Prompt),
        (AskForApproval::Granular(granular), Decision::Prompt),
        (AskForApproval::Never, Decision::Forbidden),
    ] {
        assert_eq!(
            render_decision_for_unmatched_command(
                &command,
                UnmatchedCommandContext {
                    approval_policy,
                    permission_profile: &PermissionProfile::workspace_write(),
                    windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
                    sandbox_permissions: SandboxPermissions::RequireEscalated,
                    used_complex_parsing: false,
                    command_origin: ExecPolicyCommandOrigin::Generic,
                },
            ),
            expected,
            "{approval_policy:?}",
        );
    }
}

#[tokio::test]
async fn model_resolved_posix_wrapper_composes_exact_outer_and_every_inner() {
    let outer = vec![
        host_program_path("sh"),
        "-c".to_string(),
        "echo ok".to_string(),
    ];
    let inner_allow = prefix_rule_for(&vec_str(&["echo"]), "allow");
    let full_outer_allow = prefix_rule_for(&outer, "allow");
    let short_outer_allow = prefix_rule_for(&outer[..1], "allow");
    let basename_outer_allow = prefix_rule_for(&vec_str(&["sh"]), "allow");
    let basename_outer_prompt = prefix_rule_for(&vec_str(&["sh"]), "prompt");
    let basename_outer_forbidden = prefix_rule_for(&vec_str(&["sh"]), "forbidden");

    assert_eq!(
        requirement_with_provenance(
            Some(&format!("{full_outer_allow}\n{inner_allow}")),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    );

    assert_eq!(
        requirement_with_provenance(
            Some(&format!("{short_outer_allow}\n{inner_allow}")),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
    );

    assert_eq!(
        requirement_with_provenance(
            Some(&format!("{basename_outer_allow}\n{inner_allow}")),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
    );

    assert_eq!(
        requirement_with_provenance(
            Some(&full_outer_allow),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
        "a heuristic-safe inner command is not explicit authority",
    );

    let rendered = render_shlex_command(&outer);
    assert_eq!(
        requirement_with_provenance(
            Some(&format!(
                "{full_outer_allow}\n{basename_outer_prompt}\n{inner_allow}"
            )),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::NeedsOneShotApproval {
            reason: Some(format!("`{rendered}` requires approval by policy")),
        },
        "a restrictive basename Prompt must survive an exact outer Allow",
    );
    assert_eq!(
        requirement_with_provenance(
            Some(&format!(
                "{full_outer_allow}\n{basename_outer_forbidden}\n{inner_allow}"
            )),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Forbidden {
            reason: format!("`{rendered}` rejected: policy forbids commands starting with `sh`"),
        },
        "a restrictive basename Forbidden must survive an exact outer Allow",
    );

    let path_b_program = host_absolute_path(&[
        "workspace-b",
        "bin",
        if cfg!(windows) { "sh.exe" } else { "sh" },
    ]);
    let path_b = vec![path_b_program, "-c".to_string(), "echo ok".to_string()];
    assert_eq!(
        requirement_with_provenance(
            Some(&format!("{full_outer_allow}\n{inner_allow}")),
            &path_b,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
        "an exact rule for path A must not authorize path B",
    );
}

#[tokio::test]
async fn model_resolved_posix_wrapper_uses_strictest_outer_and_inner_decision() {
    let outer = vec![
        host_program_path("bash"),
        "-lc".to_string(),
        "echo first; later value".to_string(),
    ];
    let full_outer_allow = prefix_rule_for(&outer, "allow");
    let echo_allow = prefix_rule_for(&vec_str(&["echo"]), "allow");
    let later_prompt = prefix_rule_for(&vec_str(&["later"]), "prompt");
    let later_forbidden = prefix_rule_for(&vec_str(&["later"]), "forbidden");
    let rendered = render_shlex_command(&outer);

    assert_eq!(
        requirement_with_provenance(
            Some(&format!("{full_outer_allow}\n{echo_allow}\n{later_prompt}")),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::NeedsOneShotApproval {
            reason: Some(format!("`{rendered}` requires approval by policy")),
        },
    );

    assert_eq!(
        requirement_with_provenance(
            Some(&format!(
                "{full_outer_allow}\n{echo_allow}\n{later_forbidden}"
            )),
            &outer,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Forbidden {
            reason: format!("`{rendered}` rejected: policy forbids commands starting with `later`"),
        },
    );
}

#[tokio::test]
async fn nested_posix_wrapper_cannot_hide_an_inner_forbidden_rule() {
    let command = vec_str(&["bash", "-lc", "bash -lc 'rm -rf target'"]);
    let rm_forbidden = prefix_rule_for(&vec_str(&["rm"]), "forbidden");

    assert_eq!(
        requirement_with_provenance(
            Some(&rm_forbidden),
            &command,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Forbidden {
            reason: format!(
                "`{}` rejected: policy forbids commands starting with `rm`",
                render_shlex_command(&command)
            ),
        },
    );
}

#[tokio::test]
async fn incomplete_untrusted_posix_analysis_is_one_shot_without_rules_and_terminal_with_rules() {
    let opaque = vec_str(&["/workspace/bin/sh", "-c", "echo hello > marker.txt"]);

    assert_eq!(
        requirement_with_provenance(
            /*policy_src*/ None,
            &opaque,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
    );

    assert!(matches!(
        requirement_with_provenance(
            /*policy_src*/ None,
            &opaque,
            AskForApproval::Never,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Forbidden { .. }
    ));

    let unrelated_rule = prefix_rule_for(&vec_str(&["unrelated"]), "allow");
    assert_eq!(
        requirement_with_provenance(
            Some(&unrelated_rule),
            &opaque,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Forbidden {
            reason: format!(
                "`{}` rejected: cannot completely inspect an untrusted shell wrapper while command policy rules are active",
                render_shlex_command(&opaque)
            ),
        },
    );
}

#[tokio::test]
async fn configured_incomplete_posix_body_cannot_hide_a_forbidden_descendant() {
    let rm_forbidden = prefix_rule_for(&vec_str(&["rm"]), "forbidden");
    for command in [
        vec_str(&[
            "/bin/bash",
            "-lc",
            "echo ok > out; bash -lc 'rm -rf target'",
        ]),
        vec_str(&[
            "/bin/bash",
            "-lc",
            "for target in one; do rm -rf \"$target\"; done",
        ]),
        vec_str(&["/bin/bash", "-lc", r#"find . "$FLAGS" rm -rf {} ';' > out"#]),
        vec_str(&["/bin/bash", "-lc", r#"env "$ARGS" > out"#]),
    ] {
        assert!(matches!(
            requirement_with_provenance(
                Some(&rm_forbidden),
                &command,
                AskForApproval::OnRequest,
                PermissionProfile::workspace_write(),
                SandboxPermissions::UseDefault,
                ShellApprovalProvenance::configured(),
            )
            .await,
            ExecApprovalRequirement::Forbidden { .. }
        ));

        assert!(matches!(
            requirement_with_provenance(
                /*policy_src*/ None,
                &command,
                AskForApproval::OnRequest,
                PermissionProfile::workspace_write(),
                SandboxPermissions::UseDefault,
                ShellApprovalProvenance::configured(),
            )
            .await,
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                ..
            }
        ));
    }
}

#[tokio::test]
async fn opaque_non_posix_model_runtime_is_one_shot_without_rules_and_terminal_with_rules() {
    let commands = [vec_str(&["cmd.exe", "/C", "echo hello"]), {
        #[cfg(not(windows))]
        {
            vec_str(&["pwsh", "-Command", "Write-Output hello"])
        }
        #[cfg(windows)]
        {
            vec_str(&["cmd.exe", "/D", "/C", "echo hello"])
        }
    }];

    for command in commands {
        assert_eq!(
            requirement_with_provenance(
                /*policy_src*/ None,
                &command,
                AskForApproval::OnRequest,
                PermissionProfile::workspace_write(),
                SandboxPermissions::UseDefault,
                ShellApprovalProvenance::local_model_resolved(),
            )
            .await,
            ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
            "{command:?}",
        );

        let unrelated_rule = prefix_rule_for(&vec_str(&["unrelated"]), "allow");
        assert!(
            matches!(
                requirement_with_provenance(
                    Some(&unrelated_rule),
                    &command,
                    AskForApproval::OnRequest,
                    PermissionProfile::workspace_write(),
                    SandboxPermissions::UseDefault,
                    ShellApprovalProvenance::local_model_resolved(),
                )
                .await,
                ExecApprovalRequirement::Forbidden { .. }
            ),
            "{command:?}",
        );
    }
}

#[tokio::test]
async fn incomplete_untrusted_posix_analysis_respects_granular_sandbox_approval() {
    let opaque = vec_str(&["/workspace/bin/sh", "-c", "echo hi > marker"]);

    for (sandbox_approval, expected) in [
        (
            false,
            ExecApprovalRequirement::Forbidden {
                reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
            },
        ),
        (
            true,
            ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
        ),
    ] {
        assert_eq!(
            requirement_with_provenance(
                /*policy_src*/ None,
                &opaque,
                AskForApproval::Granular(GranularApprovalConfig {
                    sandbox_approval,
                    rules: false,
                    skill_approval: true,
                    request_permissions: true,
                    mcp_elicitations: true,
                }),
                PermissionProfile::workspace_write(),
                SandboxPermissions::UseDefault,
                ShellApprovalProvenance::local_model_resolved(),
            )
            .await,
            expected,
            "sandbox_approval={sandbox_approval}",
        );
    }
}

#[tokio::test]
async fn delegators_fail_closed_but_literal_shell_name_arguments_remain_complete() {
    let echo_allow = prefix_rule_for(&vec_str(&["echo"]), "allow");
    for script in [
        "env bash -c 'echo hidden'",
        "command ./bash -c 'echo hidden'",
        "exec bash -c 'echo hidden'",
        "eval 'echo hidden'",
        ". ./payload.sh",
        "source ./payload.sh",
        "trap 'rm -rf target' EXIT",
        "xargs rm -rf",
        "find . -name target -exec rm -rf {} ';'",
        "dash -c 'rm -rf target'",
        "env FOO=bar -u bash",
        r#""$CMD" arg"#,
    ] {
        let delegated = vec![
            "/workspace/bin/sh".to_string(),
            "-c".to_string(),
            script.to_string(),
        ];
        for provenance in [
            ShellApprovalProvenance::local_model_resolved(),
            ShellApprovalProvenance::configured(),
        ] {
            assert!(
                matches!(
                    requirement_with_provenance(
                        Some(&echo_allow),
                        &delegated,
                        AskForApproval::OnRequest,
                        PermissionProfile::workspace_write(),
                        SandboxPermissions::UseDefault,
                        provenance,
                    )
                    .await,
                    ExecApprovalRequirement::Forbidden { .. }
                ),
                "{script}, {provenance:?}",
            );
        }
    }

    let literal = vec![
        host_program_path("sh"),
        "-c".to_string(),
        "echo bash".to_string(),
    ];
    let full_outer_allow = prefix_rule_for(&literal, "allow");
    assert_eq!(
        requirement_with_provenance(
            Some(&format!("{full_outer_allow}\n{echo_allow}")),
            &literal,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    );
}

#[tokio::test]
async fn configured_delegators_without_rules_preserve_legacy_sandboxed_behavior() {
    for script in [
        "env echo hi",
        "sudo echo hi",
        "xargs echo",
        "find . -exec echo {} ';'",
    ] {
        let command = vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            script.to_string(),
        ];
        for provenance in [
            ShellApprovalProvenance::configured(),
            ShellApprovalProvenance::remote_model_hint(),
        ] {
            assert!(
                matches!(
                    requirement_with_provenance(
                        /*policy_src*/ None,
                        &command,
                        AskForApproval::OnRequest,
                        PermissionProfile::workspace_write(),
                        SandboxPermissions::UseDefault,
                        provenance,
                    )
                    .await,
                    ExecApprovalRequirement::Skip {
                        bypass_sandbox: false,
                        ..
                    }
                ),
                "{script}, {provenance:?}",
            );
        }

        assert_eq!(
            requirement_with_provenance(
                /*policy_src*/ None,
                &command,
                AskForApproval::OnRequest,
                PermissionProfile::workspace_write(),
                SandboxPermissions::UseDefault,
                ShellApprovalProvenance::local_model_resolved(),
            )
            .await,
            ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
            "{script}",
        );
    }
}

#[tokio::test]
async fn configured_exact_allow_can_authorize_an_opaque_nested_shell_only() {
    let inner = vec![
        posix_script_program_path("sh"),
        "/tmp/approved-script".to_string(),
    ];
    let command = vec![
        host_program_path("zsh"),
        "-lc".to_string(),
        shlex_try_join(inner.iter().map(String::as_str)).expect("quote nested shell command"),
    ];
    let inner_allow = prefix_rule_for(&inner, "allow");

    assert_eq!(
        requirement_with_provenance(
            Some(&inner_allow),
            &command,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::RequireEscalated,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    );

    let outer_allow = prefix_rule_for(&command, "allow");
    assert!(matches!(
        requirement_with_provenance(
            Some(&format!("{outer_allow}\n{inner_allow}")),
            &command,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::RequireEscalated,
            ShellApprovalProvenance::local_model_resolved(),
        )
        .await,
        ExecApprovalRequirement::Forbidden { .. }
    ));
}

#[tokio::test]
async fn bare_nested_shell_allow_never_establishes_wrapper_authority() {
    let outer_shell = host_program_path("zsh");
    let bare_wrapper = vec_str(&["bash", "-lc", "echo hello"]);
    let bare = vec![
        outer_shell.clone(),
        "-lc".to_string(),
        shlex_try_join(bare_wrapper.iter().map(String::as_str)).expect("quote bare nested wrapper"),
    ];
    let echo_allow = prefix_rule_for(&vec_str(&["echo"]), "allow");
    let bare_rules = format!("{}\n{echo_allow}", prefix_rule_for(&bare_wrapper, "allow"));

    assert_eq!(
        requirement_with_provenance(
            Some(&bare_rules),
            &bare,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
    );

    assert_eq!(
        requirement_with_provenance(
            Some(&bare_rules),
            &bare,
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            PermissionProfile::workspace_write(),
            SandboxPermissions::RequireEscalated,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Forbidden {
            reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
        },
    );

    let relative_wrapper = vec_str(&["./bash", "-lc", "echo hello"]);
    let relative = vec![
        outer_shell.clone(),
        "-lc".to_string(),
        shlex_try_join(relative_wrapper.iter().map(String::as_str))
            .expect("quote relative nested wrapper"),
    ];
    let relative_rules = format!(
        "{}\n{echo_allow}",
        prefix_rule_for(&relative_wrapper, "allow")
    );
    assert_eq!(
        requirement_with_provenance(
            Some(&relative_rules),
            &relative,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
    );

    assert_eq!(
        requirement_with_provenance(
            Some(&relative_rules),
            &relative,
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            PermissionProfile::workspace_write(),
            SandboxPermissions::RequireEscalated,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Forbidden {
            reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
        },
    );

    let absolute_wrapper = vec![
        posix_script_program_path("bash"),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];
    let absolute = vec![
        outer_shell.clone(),
        "-lc".to_string(),
        shlex_try_join(absolute_wrapper.iter().map(String::as_str))
            .expect("quote absolute nested wrapper"),
    ];
    let absolute_rules = format!(
        "{}\n{echo_allow}",
        prefix_rule_for(&absolute_wrapper, "allow")
    );
    assert_eq!(
        requirement_with_provenance(
            Some(&absolute_rules),
            &absolute,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::RequireEscalated,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    );

    let opaque_bare = vec![outer_shell, "-lc".to_string(), "bash".to_string()];
    let opaque_bare_allow = prefix_rule_for(&vec_str(&["bash"]), "allow");
    assert!(matches!(
        requirement_with_provenance(
            Some(&opaque_bare_allow),
            &opaque_bare,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Forbidden { .. }
    ));
}

#[test]
fn wrapper_authority_uses_controller_native_absolute_paths() {
    assert!(executable_spelling_is_absolute(&host_program_path("bash")));

    let foreign_absolute = if cfg!(windows) {
        "/usr/bin/bash"
    } else {
        r"C:\Program Files\Git\bin\bash.exe"
    };
    assert!(
        !executable_spelling_is_absolute(foreign_absolute),
        "foreign-target absolute paths conservatively cannot establish bypass authority"
    );
}

#[test]
fn posix_analysis_is_bounded_and_plain_find_is_not_a_delegator() {
    for query in [
        vec_str(&["command", "-v", "bash"]),
        vec_str(&["trap", "-p"]),
    ] {
        assert!(
            !command_may_hide_nested_execution(&query),
            "query-only form was classified as a delegator: {query:?}"
        );
    }
    for external in [
        vec_str(&["/repo/command", "-v", "bash"]),
        vec_str(&["./trap", "-p"]),
        vec_str(&["command.exe", "-v", "bash"]),
        vec_str(&["COMMAND", "-v", "bash"]),
    ] {
        assert!(
            command_may_hide_nested_execution(&external),
            "external executable was treated as a shell builtin: {external:?}"
        );
    }

    let mut nested_script = "echo leaf".to_string();
    for _ in 0..=MAX_POSIX_POLICY_DEPTH {
        nested_script = format!(
            "bash -lc {}",
            shlex_try_join([nested_script.as_str()]).expect("quote nested script")
        );
    }
    let depth_limited = vec!["/bin/bash".to_string(), "-lc".to_string(), nested_script];
    let depth_analysis = analyze_posix_policy(
        &depth_limited,
        ShellApprovalProvenance::local_model_resolved(),
    )
    .expect("POSIX analysis");
    assert_eq!(
        depth_analysis.completeness,
        PosixAnalysisCompleteness::Incomplete
    );
    assert!(depth_analysis.contains_untrusted_wrapper);

    let candidate_script = (0..=MAX_POSIX_POLICY_CANDIDATES)
        .map(|index| format!("echo value{index}"))
        .collect::<Vec<_>>()
        .join("; ");
    let candidate_limited = vec!["/bin/bash".to_string(), "-lc".to_string(), candidate_script];
    let candidate_analysis =
        analyze_posix_policy(&candidate_limited, ShellApprovalProvenance::configured())
            .expect("POSIX analysis");
    assert_eq!(
        candidate_analysis.completeness,
        PosixAnalysisCompleteness::Incomplete
    );
    assert_eq!(
        candidate_analysis.candidates.len(),
        MAX_POSIX_POLICY_CANDIDATES
    );

    let oversized = vec![
        "/bin/bash".to_string(),
        "-lc".to_string(),
        "x".repeat(MAX_POSIX_POLICY_SCRIPT_BYTES + 1),
    ];
    let oversized_analysis =
        analyze_posix_policy(&oversized, ShellApprovalProvenance::local_model_resolved())
            .expect("POSIX analysis");
    assert_eq!(
        oversized_analysis.completeness,
        PosixAnalysisCompleteness::Incomplete
    );

    let plain_find = vec_str(&["/bin/bash", "-lc", "find . -name target"]);
    let plain_find_analysis =
        analyze_posix_policy(&plain_find, ShellApprovalProvenance::configured())
            .expect("POSIX analysis");
    assert_eq!(
        plain_find_analysis.completeness,
        PosixAnalysisCompleteness::Complete
    );
    assert!(!plain_find_analysis.contains_untrusted_wrapper);
}

#[tokio::test]
async fn remote_model_hint_does_not_distrust_the_environment_runtime_but_never_amends() {
    let safe = vec_str(&["echo", "hello"]);
    assert_eq!(
        requirement_with_provenance(
            /*policy_src*/ None,
            &safe,
            AskForApproval::OnRequest,
            PermissionProfile::workspace_write(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::remote_model_hint(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: None,
        },
    );

    let unsafe_command = vec_str(&["cargo", "build"]);
    assert_eq!(
        requirement_with_provenance(
            /*policy_src*/ None,
            &unsafe_command,
            AskForApproval::UnlessTrusted,
            PermissionProfile::read_only(),
            SandboxPermissions::UseDefault,
            ShellApprovalProvenance::remote_model_hint(),
        )
        .await,
        ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
    );
}

#[tokio::test]
async fn model_resolved_rule_and_permission_prompts_require_both_granular_categories() {
    let command = vec_str(&["/workspace/bin/sh", "-c", "echo hello"]);
    let policy_src = format!(
        "{}\n{}\n{}",
        prefix_rule_for(&command, "allow"),
        prefix_rule_for(&vec_str(&["echo"]), "allow"),
        prefix_rule_for(&vec_str(&["echo", "hello"]), "prompt"),
    );
    let rendered = render_shlex_command(&command);

    for (rules, sandbox_approval, expected) in [
        (
            false,
            true,
            ExecApprovalRequirement::Forbidden {
                reason: REJECT_RULES_APPROVAL_REASON.to_string(),
            },
        ),
        (
            true,
            false,
            ExecApprovalRequirement::Forbidden {
                reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
            },
        ),
        (
            true,
            true,
            ExecApprovalRequirement::NeedsOneShotApproval {
                reason: Some(format!("`{rendered}` requires approval by policy")),
            },
        ),
    ] {
        assert_eq!(
            requirement_with_provenance(
                Some(&policy_src),
                &command,
                AskForApproval::Granular(GranularApprovalConfig {
                    sandbox_approval,
                    rules,
                    skill_approval: true,
                    request_permissions: true,
                    mcp_elicitations: true,
                }),
                PermissionProfile::workspace_write(),
                SandboxPermissions::RequireEscalated,
                ShellApprovalProvenance::local_model_resolved(),
            )
            .await,
            expected,
            "rules={rules}, sandbox_approval={sandbox_approval}",
        );
    }
}

#[tokio::test]
async fn model_selected_requested_prefix_cannot_reenable_an_amendment() {
    let command = vec_str(&["/workspace/bin/sh", "-c", "echo hello"]);
    let requirement = ExecPolicyManager::default()
        .create_exec_approval_requirement_for_command_with_provenance(
            ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::OnRequest,
                permission_profile: PermissionProfile::workspace_write(),
                windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: Some(vec_str(&["echo"])),
            },
            ShellApprovalProvenance::local_model_resolved(),
            /*permission_expansion_was_requested*/ false,
        )
        .await;

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsOneShotApproval { reason: None },
    );
}

#[tokio::test]
async fn preapproved_permission_expansion_cannot_generate_a_sticky_amendment() {
    let command = vec_str(&["cargo", "build"]);
    let manager = ExecPolicyManager::default();
    for (sandbox_permissions, permission_expansion_was_requested) in [
        (SandboxPermissions::UseDefault, true),
        (SandboxPermissions::RequireEscalated, false),
    ] {
        let requirement = manager
            .create_exec_approval_requirement_for_configured_command(
                ExecApprovalRequest {
                    command: &command,
                    approval_policy: AskForApproval::UnlessTrusted,
                    permission_profile: PermissionProfile::read_only(),
                    windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
                    sandbox_permissions,
                    prefix_rule: Some(vec_str(&["cargo"])),
                },
                permission_expansion_was_requested,
            )
            .await;

        assert_eq!(
            requirement,
            ExecApprovalRequirement::NeedsApproval {
                reason: None,
                proposed_execpolicy_amendment: None,
            },
            "sandbox_permissions={sandbox_permissions:?}, permission_expansion_was_requested={permission_expansion_was_requested}",
        );
    }
}

#[test]
fn managed_cwd_write_profile_has_filesystem_restrictions() {
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Write,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    assert!(profile_has_managed_filesystem_restrictions(
        &permission_profile
    ));
}

#[test]
fn managed_unresolvable_write_profile_has_filesystem_restrictions() {
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::unknown(
                    ":future_special_path",
                    /*subpath*/ None,
                ),
            },
            access: FileSystemAccessMode::Write,
        },
    ]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    assert!(profile_has_managed_filesystem_restrictions(
        &permission_profile
    ));
}

#[test]
fn managed_full_disk_write_profile_has_no_filesystem_restrictions() {
    let file_system_sandbox_policy =
        FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        }]);
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );

    assert!(!profile_has_managed_filesystem_restrictions(
        &permission_profile
    ));
}

#[tokio::test]
async fn exec_approval_requirement_prompts_for_inline_additional_permissions_under_on_request() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                "zsh".to_string(),
                "-lc".to_string(),
                "touch requested-dir/requested-but-unused.txt".to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::WithAdditionalPermissions,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn exec_approval_requirement_prompts_for_known_safe_escalation_under_on_request() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec!["echo".to_string(), "hello".to_string()],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::workspace_write(),
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn exec_approval_requirement_rejects_known_safe_escalation_when_granular_sandbox_is_disabled()
{
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec!["echo".to_string(), "hello".to_string()],
            approval_policy: AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            permission_profile: PermissionProfile::workspace_write(),
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Forbidden {
            reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
        },
    )
    .await;
}

#[tokio::test]
async fn exec_approval_requirement_rejects_unmatched_sandbox_escalation_when_granular_sandbox_is_disabled()
 {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec!["madeup-cmd".to_string()],
            approval_policy: AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Forbidden {
            reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
        },
    )
    .await;
}

#[tokio::test]
async fn mixed_rule_and_sandbox_prompt_requires_every_granular_category_in_either_order() {
    let policy_src = r#"prefix_rule(pattern=["git"], decision="prompt")"#;
    let mut parser = PolicyParser::new();
    parser
        .parse("test.rules", policy_src)
        .expect("parse policy");
    let manager = ExecPolicyManager::new(Arc::new(parser.build()));
    let cases = [
        ("rule then sandbox", "git status && madeup-cmd"),
        ("sandbox then rule", "madeup-cmd && git status"),
    ];

    for (order, script) in cases {
        let command = vec!["bash".to_string(), "-lc".to_string(), script.to_string()];
        for (rules, sandbox_approval, expected_rejection) in [
            (false, false, Some(REJECT_RULES_APPROVAL_REASON)),
            (false, true, Some(REJECT_RULES_APPROVAL_REASON)),
            (true, false, Some(REJECT_SANDBOX_APPROVAL_REASON)),
            (true, true, None),
        ] {
            let requirement = manager
                .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                    command: &command,
                    approval_policy: AskForApproval::Granular(GranularApprovalConfig {
                        sandbox_approval,
                        rules,
                        skill_approval: true,
                        request_permissions: true,
                        mcp_elicitations: true,
                    }),
                    permission_profile: PermissionProfile::read_only(),
                    windows_sandbox_level: WindowsSandboxLevel::Disabled,
                    sandbox_permissions: SandboxPermissions::RequireEscalated,
                    prefix_rule: None,
                })
                .await;
            let expected = match expected_rejection {
                Some(reason) => ExecApprovalRequirement::Forbidden {
                    reason: reason.to_string(),
                },
                None => ExecApprovalRequirement::NeedsApproval {
                    reason: Some(format!(
                        "`{}` requires approval by policy",
                        render_shlex_command(&command)
                    )),
                    proposed_execpolicy_amendment: None,
                },
            };

            assert_eq!(
                requirement, expected,
                "{order} with rules={rules} and sandbox_approval={sandbox_approval}",
            );
        }
    }
}

#[tokio::test]
async fn same_command_policy_prompt_and_sandbox_override_require_both_categories() {
    let command = vec_str(&["git", "status"]);
    let prompt_policies = [
        prefix_rule_for(&vec_str(&["git"]), "prompt"),
        format!(
            "{}\n{}",
            prefix_rule_for(&vec_str(&["git"]), "allow"),
            prefix_rule_for(&command, "prompt"),
        ),
        format!(
            "{}\n{}",
            prefix_rule_for(&vec_str(&["git"]), "prompt"),
            prefix_rule_for(&command, "allow"),
        ),
    ];

    for policy_src in prompt_policies {
        for (rules, sandbox_approval, expected) in [
            (
                false,
                true,
                ExecApprovalRequirement::Forbidden {
                    reason: REJECT_RULES_APPROVAL_REASON.to_string(),
                },
            ),
            (
                true,
                false,
                ExecApprovalRequirement::Forbidden {
                    reason: REJECT_SANDBOX_APPROVAL_REASON.to_string(),
                },
            ),
            (
                true,
                true,
                ExecApprovalRequirement::NeedsApproval {
                    reason: Some(format!(
                        "`{}` requires approval by policy",
                        render_shlex_command(&command)
                    )),
                    proposed_execpolicy_amendment: None,
                },
            ),
        ] {
            assert_eq!(
                requirement_with_provenance(
                    Some(&policy_src),
                    &command,
                    AskForApproval::Granular(GranularApprovalConfig {
                        sandbox_approval,
                        rules,
                        skill_approval: true,
                        request_permissions: true,
                        mcp_elicitations: true,
                    }),
                    PermissionProfile::workspace_write(),
                    SandboxPermissions::RequireEscalated,
                    ShellApprovalProvenance::configured(),
                )
                .await,
                expected,
                "policy={policy_src:?}, rules={rules}, sandbox_approval={sandbox_approval}",
            );
        }
    }

    let effective_allow = prefix_rule_for(&vec_str(&["git"]), "allow");
    assert_eq!(
        requirement_with_provenance(
            Some(&effective_allow),
            &command,
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: false,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            PermissionProfile::workspace_write(),
            SandboxPermissions::RequireEscalated,
            ShellApprovalProvenance::configured(),
        )
        .await,
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    );
}

#[tokio::test]
async fn exec_approval_requirement_falls_back_to_heuristics() {
    let command = vec!["cargo".to_string(), "build".to_string()];

    let manager = ExecPolicyManager::default();
    let requirement = manager
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        })
        .await;

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command))
        }
    );
}

#[tokio::test]
async fn empty_bash_lc_script_falls_back_to_original_command() {
    let command = vec!["bash".to_string(), "-lc".to_string(), "".to_string()];

    let manager = ExecPolicyManager::default();
    let requirement = manager
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        })
        .await;

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
        }
    );
}

#[tokio::test]
async fn whitespace_bash_lc_script_falls_back_to_original_command() {
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "  \n\t  ".to_string(),
    ];

    let manager = ExecPolicyManager::default();
    let requirement = manager
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        })
        .await;

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
        }
    );
}

#[tokio::test]
async fn request_rule_uses_prefix_rule() {
    let command = vec![
        "cargo".to_string(),
        "install".to_string(),
        "cargo-insta".to_string(),
    ];
    let manager = ExecPolicyManager::default();

    let requirement = manager
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            prefix_rule: Some(vec!["cargo".to_string(), "install".to_string()]),
        })
        .await;

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[tokio::test]
async fn request_rule_falls_back_when_prefix_rule_does_not_approve_all_commands() {
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "cargo install cargo-insta && rm -rf /tmp/codex".to_string(),
    ];
    let manager = ExecPolicyManager::default();

    let requirement = manager
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            windows_sandbox_level: WindowsSandboxLevel::Disabled,
            sandbox_permissions: SandboxPermissions::RequireEscalated,
            prefix_rule: Some(vec!["cargo".to_string(), "install".to_string()]),
        })
        .await;

    assert_eq!(
        requirement,
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    );
}

#[tokio::test]
async fn heuristics_apply_when_other_commands_match_policy() {
    let policy_src = r#"prefix_rule(pattern=["apple"], decision="allow")"#;
    let mut parser = PolicyParser::new();
    parser
        .parse("test.rules", policy_src)
        .expect("parse policy");
    let policy = Arc::new(parser.build());
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "apple | orange".to_string(),
    ];

    assert_eq!(
        ExecPolicyManager::new(policy)
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &command,
                approval_policy: AskForApproval::UnlessTrusted,
                permission_profile: PermissionProfile::Disabled,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            })
            .await,
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                "orange".to_string()
            ]))
        }
    );
}

#[tokio::test]
async fn append_execpolicy_amendment_updates_policy_and_file() {
    let codex_home = tempdir().expect("create temp dir");
    let prefix = vec!["echo".to_string(), "hello".to_string()];
    let manager = ExecPolicyManager::default();

    manager
        .append_amendment_and_update(codex_home.path(), &ExecPolicyAmendment::from(prefix))
        .await
        .expect("update policy");
    let updated_policy = manager.current();

    let evaluation = updated_policy.check(
        &["echo".to_string(), "hello".to_string(), "world".to_string()],
        &|_| Decision::Allow,
    );
    assert!(matches!(
        evaluation,
        Evaluation {
            decision: Decision::Allow,
            ..
        }
    ));

    let contents = fs::read_to_string(default_policy_path(codex_home.path()))
        .expect("policy file should have been created");
    assert_eq!(
        contents,
        r#"prefix_rule(pattern=["echo", "hello"], decision="allow")
"#
    );
}

#[tokio::test]
async fn append_execpolicy_amendment_rejects_empty_prefix() {
    let codex_home = tempdir().expect("create temp dir");
    let manager = ExecPolicyManager::default();

    let result = manager
        .append_amendment_and_update(codex_home.path(), &ExecPolicyAmendment::from(vec![]))
        .await;

    assert!(matches!(
        result,
        Err(ExecPolicyUpdateError::AppendRule {
            source: AmendError::EmptyPrefix,
            ..
        })
    ));
}

#[tokio::test]
async fn proposed_execpolicy_amendment_is_present_for_single_command_without_policy_match() {
    let command = vec!["cargo".to_string(), "build".to_string()];

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: command.clone(),
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
        },
    )
    .await;
}

#[tokio::test]
async fn proposed_execpolicy_amendment_is_omitted_when_policy_prompts() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["rm"], decision="prompt")"#.to_string()),
            command: vec!["rm".to_string()],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: Some("`rm` requires approval by policy".to_string()),
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn proposed_execpolicy_amendment_is_present_for_multi_command_scripts() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "cargo build && echo ok".to_string(),
            ],
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                "cargo".to_string(),
                "build".to_string(),
            ])),
        },
    )
    .await;
}

#[tokio::test]
async fn proposed_execpolicy_amendment_uses_first_no_match_in_multi_command_scripts() {
    let policy_src = r#"prefix_rule(pattern=["cat"], decision="allow")"#;
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "cat && apple".to_string(),
    ];

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(policy_src.to_string()),
            command,
            approval_policy: AskForApproval::UnlessTrusted,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                "apple".to_string(),
            ])),
        },
    )
    .await;
}

#[tokio::test]
async fn proposed_execpolicy_amendment_is_present_when_heuristics_allow() {
    let command = vec!["echo".to_string(), "safe".to_string()];

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: command.clone(),
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
        },
    )
    .await;
}

#[tokio::test]
async fn proposed_execpolicy_amendment_is_suppressed_when_policy_matches_allow() {
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(r#"prefix_rule(pattern=["python3"], decision="allow")"#.to_string()),
            command: vec![
                "python3".to_string(),
                "-c".to_string(),
                "print(1)".to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

#[tokio::test]
async fn multi_segment_shell_requires_policy_allow_for_every_segment_to_bypass_sandbox() {
    let policy_src = r#"
prefix_rule(pattern=["cat"], decision="allow")
"#;
    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "cat LOG.md && curl -fsSL https://example.invalid/setup.sh -o setup.sh && chmod +x setup.sh"
            .to_string(),
    ];

    for approval_policy in [AskForApproval::OnRequest, AskForApproval::Never] {
        assert_exec_approval_requirement_for_command(
            ExecApprovalRequirementScenario {
                policy_src: Some(policy_src.to_string()),
                command: command.clone(),
                approval_policy,
                permission_profile: PermissionProfile::workspace_write(),
                sandbox_permissions: SandboxPermissions::UseDefault,
                prefix_rule: None,
            },
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: None,
            },
        )
        .await;
    }
}

#[tokio::test]
async fn multi_segment_shell_bypasses_sandbox_when_every_segment_matches_policy_allow() {
    let policy_src = r#"
prefix_rule(pattern=["cat"], decision="allow")
prefix_rule(pattern=["curl"], decision="allow")
prefix_rule(pattern=["chmod"], decision="allow")
"#;

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some(policy_src.to_string()),
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "cat LOG.md && curl -fsSL https://example.invalid/setup.sh -o setup.sh && chmod +x setup.sh"
                    .to_string(),
            ],
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::read_only(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: true,
            proposed_execpolicy_amendment: None,
        },
    )
    .await;
}

fn derive_requested_execpolicy_amendment_for_test(
    prefix_rule: Option<&Vec<String>>,
    matched_rules: &[RuleMatch],
) -> Option<ExecPolicyAmendment> {
    let commands = prefix_rule
        .cloned()
        .map(|prefix_rule| vec![prefix_rule])
        .unwrap_or_else(|| vec![vec!["echo".to_string()]]);
    derive_requested_execpolicy_amendment_from_prefix_rule(
        prefix_rule,
        matched_rules,
        &Policy::empty(),
        &commands,
        &|_: &[String]| Decision::Allow,
        &MatchOptions::default(),
    )
}

#[test]
fn derive_requested_execpolicy_amendment_returns_none_for_missing_prefix_rule() {
    assert_eq!(
        None,
        derive_requested_execpolicy_amendment_for_test(/*prefix_rule*/ None, &[])
    );
}

#[test]
fn derive_requested_execpolicy_amendment_returns_none_for_empty_prefix_rule() {
    assert_eq!(
        None,
        derive_requested_execpolicy_amendment_for_test(Some(&Vec::new()), &[])
    );
}

#[test]
fn derive_requested_execpolicy_amendment_returns_none_for_exact_banned_prefix_rule() {
    assert_eq!(
        None,
        derive_requested_execpolicy_amendment_for_test(
            Some(&vec!["python".to_string(), "-c".to_string()]),
            &[],
        )
    );
}

#[test]
fn derive_requested_execpolicy_amendment_returns_none_for_windows_and_pypy_variants() {
    for prefix_rule in [
        vec!["py".to_string()],
        vec!["py".to_string(), "-3".to_string()],
        vec!["pythonw".to_string()],
        vec!["pyw".to_string()],
        vec!["pypy".to_string()],
        vec!["pypy3".to_string()],
    ] {
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_for_test(Some(&prefix_rule), &[])
        );
    }
}

#[test]
fn derive_requested_execpolicy_amendment_returns_none_for_shell_and_powershell_variants() {
    for prefix_rule in [
        vec!["bash".to_string(), "-lc".to_string()],
        vec!["sh".to_string(), "-c".to_string()],
        vec!["sh".to_string(), "-lc".to_string()],
        vec!["zsh".to_string(), "-lc".to_string()],
        vec!["/bin/bash".to_string(), "-lc".to_string()],
        vec!["/bin/zsh".to_string(), "-lc".to_string()],
        vec!["pwsh".to_string()],
        vec!["pwsh".to_string(), "-Command".to_string()],
        vec!["pwsh".to_string(), "-c".to_string()],
        vec!["powershell".to_string()],
        vec!["powershell".to_string(), "-Command".to_string()],
        vec!["powershell".to_string(), "-c".to_string()],
        vec!["powershell.exe".to_string()],
        vec!["powershell.exe".to_string(), "-Command".to_string()],
        vec!["powershell.exe".to_string(), "-c".to_string()],
    ] {
        assert_eq!(
            None,
            derive_requested_execpolicy_amendment_for_test(Some(&prefix_rule), &[])
        );
    }
}

#[test]
fn derive_requested_execpolicy_amendment_allows_non_exact_banned_prefix_rule_match() {
    let prefix_rule = vec![
        "python".to_string(),
        "-c".to_string(),
        "print('hi')".to_string(),
    ];

    assert_eq!(
        Some(ExecPolicyAmendment::new(prefix_rule.clone())),
        derive_requested_execpolicy_amendment_for_test(Some(&prefix_rule), &[])
    );
}

#[test]
fn derive_requested_execpolicy_amendment_returns_none_when_policy_matches() {
    let prefix_rule = vec!["cargo".to_string(), "build".to_string()];

    let matched_rules_prompt = vec![RuleMatch::PrefixRuleMatch {
        matched_prefix: vec!["cargo".to_string()],
        decision: Decision::Prompt,
        resolved_program: None,
        justification: None,
    }];
    assert_eq!(
        None,
        derive_requested_execpolicy_amendment_for_test(Some(&prefix_rule), &matched_rules_prompt),
        "should return none when prompt policy matches"
    );
    let matched_rules_allow = vec![RuleMatch::PrefixRuleMatch {
        matched_prefix: vec!["cargo".to_string()],
        decision: Decision::Allow,
        resolved_program: None,
        justification: None,
    }];
    assert_eq!(
        None,
        derive_requested_execpolicy_amendment_for_test(Some(&prefix_rule), &matched_rules_allow),
        "should return none when prompt policy matches"
    );
    let matched_rules_forbidden = vec![RuleMatch::PrefixRuleMatch {
        matched_prefix: vec!["cargo".to_string()],
        decision: Decision::Forbidden,
        resolved_program: None,
        justification: None,
    }];
    assert_eq!(
        None,
        derive_requested_execpolicy_amendment_for_test(
            Some(&prefix_rule),
            &matched_rules_forbidden,
        ),
        "should return none when prompt policy matches"
    );
}

#[tokio::test]
async fn dangerous_rm_rf_requires_approval_in_danger_full_access() {
    let command = vec_str(&["rm", "-rf", "/tmp/nonexistent"]);

    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command: command.clone(),
            approval_policy: AskForApproval::OnRequest,
            permission_profile: PermissionProfile::Disabled,
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(command)),
        },
    )
    .await;
}

fn vec_str(items: &[&str]) -> Vec<String> {
    items.iter().map(std::string::ToString::to_string).collect()
}

/// Note this test behaves differently on Windows because it exercises an
/// `if cfg!(windows)` code path in render_decision_for_unmatched_command().
#[tokio::test]
async fn verify_approval_requirement_for_unsafe_powershell_command() {
    // `brew install powershell` to run this test on a Mac!
    // Note `pwsh` is required to parse a PowerShell command to see if it
    // is safe.
    if which::which("pwsh").is_err() {
        return;
    }

    let policy = ExecPolicyManager::new(Arc::new(Policy::empty()));
    let permissions = SandboxPermissions::UseDefault;

    // This command should not be run without user approval unless there is
    // a proper sandbox in place to ensure safety.
    let sneaky_command = vec_str(&["pwsh", "-Command", "echo hi @(calc)"]);
    let expected_amendment = Some(ExecPolicyAmendment::new(vec_str(&[
        "pwsh",
        "-Command",
        "echo hi @(calc)",
    ])));
    let (pwsh_approval_reason, expected_req) = if cfg!(windows) {
        (
            "On Windows, an untrusted PowerShell wrapper that cannot be inspected is rejected.",
            ExecApprovalRequirement::Forbidden {
                reason: format!(
                    "`{}` rejected: an untrusted PowerShell wrapper could not be inspected with \
                     the protected system parser",
                    render_shlex_command(&sneaky_command)
                ),
            },
        )
    } else {
        (
            "On non-Windows, rely on the read-only sandbox to prevent harm.",
            ExecApprovalRequirement::Skip {
                bypass_sandbox: false,
                proposed_execpolicy_amendment: expected_amendment.clone(),
            },
        )
    };
    assert_eq!(
        expected_req,
        policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &sneaky_command,
                approval_policy: AskForApproval::OnRequest,
                permission_profile: PermissionProfile::read_only(),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: permissions,
                prefix_rule: None,
            })
            .await,
        "{pwsh_approval_reason}"
    );

    // This is flagged as a dangerous command on all platforms.
    let dangerous_command = vec_str(&["rm", "-rf", "/important/data"]);
    assert_eq!(
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec_str(&[
                "rm",
                "-rf",
                "/important/data",
            ]))),
        },
        policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &dangerous_command,
                approval_policy: AskForApproval::OnRequest,
                permission_profile: PermissionProfile::read_only(),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: permissions,
                prefix_rule: None,
            })
            .await,
        r#"On all platforms, a forbidden command should require approval
            (unless AskForApproval::Never is specified)."#
    );

    // A dangerous command should be forbidden if the user has specified
    // AskForApproval::Never.
    assert_eq!(
        ExecApprovalRequirement::Forbidden {
            reason: "`rm -rf /important/data` rejected: blocked by policy".to_string(),
        },
        policy
            .create_exec_approval_requirement_for_command(ExecApprovalRequest {
                command: &dangerous_command,
                approval_policy: AskForApproval::Never,
                permission_profile: PermissionProfile::read_only(),
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                sandbox_permissions: permissions,
                prefix_rule: None,
            })
            .await,
        r#"On all platforms, a forbidden command should require approval
            (unless AskForApproval::Never is specified)."#
    );
}

#[tokio::test]
async fn dangerous_command_allowed_when_sandbox_is_explicitly_disabled() {
    let command = vec_str(&["rm", "-rf", "/tmp/nonexistent"]);
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: None,
            command,
            approval_policy: AskForApproval::Never,
            permission_profile: PermissionProfile::External {
                network: NetworkSandboxPolicy::Restricted,
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Skip {
            bypass_sandbox: false,
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment {
                command: vec_str(&["rm", "-rf", "/tmp/nonexistent"]),
            }),
        },
    )
    .await;
}

#[tokio::test]
async fn dangerous_command_forbidden_in_external_sandbox_when_policy_matches() {
    let command = vec_str(&["rm", "-rf", "/tmp/nonexistent"]);
    assert_exec_approval_requirement_for_command(
        ExecApprovalRequirementScenario {
            policy_src: Some("prefix_rule(pattern=['rm'], decision='prompt')".to_string()),
            command,
            approval_policy: AskForApproval::Never,
            permission_profile: PermissionProfile::External {
                network: NetworkSandboxPolicy::Restricted,
            },
            sandbox_permissions: SandboxPermissions::UseDefault,
            prefix_rule: None,
        },
        ExecApprovalRequirement::Forbidden {
            reason: "approval required by policy, but AskForApproval is set to Never".to_string(),
        },
    )
    .await;
}

struct ExecApprovalRequirementScenario {
    /// Source for the Starlark `.rules` file.
    policy_src: Option<String>,
    command: Vec<String>,
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    sandbox_permissions: SandboxPermissions,
    prefix_rule: Option<Vec<String>>,
}

fn policy_from_src(policy_src: Option<&str>) -> Arc<Policy> {
    match policy_src {
        Some(src) => {
            let mut parser = PolicyParser::new();
            parser.parse("test.rules", src).expect("parse policy");
            Arc::new(parser.build())
        }
        None => Arc::new(Policy::empty()),
    }
}

async fn exec_approval_requirement_for_command(
    test: ExecApprovalRequirementScenario,
) -> ExecApprovalRequirement {
    let ExecApprovalRequirementScenario {
        policy_src,
        command,
        approval_policy,
        permission_profile,
        sandbox_permissions,
        prefix_rule,
    } = test;

    let policy = policy_from_src(policy_src.as_deref());

    ExecPolicyManager::new(policy)
        .create_exec_approval_requirement_for_command(ExecApprovalRequest {
            command: &command,
            approval_policy,
            permission_profile,
            windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
            sandbox_permissions,
            prefix_rule,
        })
        .await
}

async fn assert_exec_approval_requirement_for_command(
    test: ExecApprovalRequirementScenario,
    expected_requirement: ExecApprovalRequirement,
) {
    let requirement = exec_approval_requirement_for_command(test).await;
    assert_eq!(requirement, expected_requirement);
}

#[tokio::test]
async fn exec_policies_only_load_from_trusted_project_layers() -> std::io::Result<()> {
    let temp = tempfile::tempdir()?;
    let codex_home = temp.path().join("home_execpolicy_nested");
    let project_root = temp.path().join("project_execpolicy_nested");
    let nested = project_root.join("nested");
    let root_rules = project_root.join(".codex").join(RULES_DIR_NAME);
    let nested_rules = nested.join(".codex").join(RULES_DIR_NAME);

    fs::create_dir_all(&codex_home)?;
    fs::create_dir_all(&nested_rules)?;
    fs::write(project_root.join(".git"), "gitdir: here")?;
    fs::create_dir_all(&root_rules)?;
    fs::write(
        root_rules.join("deny-rm.rules"),
        r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
    )?;
    fs::write(
        nested_rules.join("deny-mv.rules"),
        r#"prefix_rule(pattern=["mv"], decision="forbidden")"#,
    )?;
    write_project_trust_config(&codex_home, &[(&nested, TrustLevel::Trusted)]).await?;

    let config = ConfigBuilder::default()
        .codex_home(codex_home)
        .fallback_cwd(Some(nested))
        .build()
        .await?;

    let policy = load_exec_policy(&config.config_layer_stack)
        .await
        .map_err(std::io::Error::other)?;
    assert_eq!(
        policy
            .check_multiple([vec!["rm".to_string()]].iter(), &|_| Decision::Allow)
            .decision,
        Decision::Allow,
    );
    assert_eq!(
        policy
            .check_multiple([vec!["mv".to_string()]].iter(), &|_| Decision::Allow)
            .decision,
        Decision::Forbidden,
    );

    Ok(())
}

#[tokio::test]
async fn exec_policies_require_project_trust_without_config_toml() -> std::io::Result<()> {
    let temp = tempfile::tempdir()?;
    let project_root = temp.path().join("project_execpolicy");
    let nested = project_root.join("nested");
    let rules_dir = project_root.join(".codex").join(RULES_DIR_NAME);
    fs::create_dir_all(&nested)?;
    fs::write(project_root.join(".git"), "gitdir: here")?;
    fs::create_dir_all(&rules_dir)?;
    fs::write(
        rules_dir.join("deny-rm.rules"),
        r#"prefix_rule(pattern=["rm"], decision="forbidden")"#,
    )?;

    let cases = [
        (
            "unknown",
            Vec::<(&Path, TrustLevel)>::new(),
            Decision::Allow,
        ),
        (
            "untrusted",
            vec![(&project_root as &Path, TrustLevel::Untrusted)],
            Decision::Allow,
        ),
        (
            "trusted",
            vec![(&project_root as &Path, TrustLevel::Trusted)],
            Decision::Forbidden,
        ),
    ];

    for (name, trust_entries, expected_decision) in cases {
        let codex_home = temp.path().join(format!("home_execpolicy_{name}"));
        fs::create_dir_all(&codex_home)?;
        write_project_trust_config(&codex_home, &trust_entries).await?;

        let config = ConfigBuilder::default()
            .codex_home(codex_home)
            .fallback_cwd(Some(nested.clone()))
            .build()
            .await?;

        let policy = load_exec_policy(&config.config_layer_stack)
            .await
            .map_err(std::io::Error::other)?;
        assert_eq!(
            policy
                .check_multiple([vec!["rm".to_string()]].iter(), &|_| Decision::Allow)
                .decision,
            expected_decision,
            "unexpected execpolicy decision for {name}",
        );
    }

    Ok(())
}

#[tokio::test]
async fn exec_policy_warnings_ignore_untrusted_project_rules_without_config_toml()
-> std::io::Result<()> {
    let temp = tempfile::tempdir()?;
    let project_root = temp.path().join("project_execpolicy_warning");
    let nested = project_root.join("nested");
    let rules_dir = project_root.join(".codex").join(RULES_DIR_NAME);
    fs::create_dir_all(&nested)?;
    fs::write(project_root.join(".git"), "gitdir: here")?;
    fs::create_dir_all(&rules_dir)?;
    fs::write(rules_dir.join("broken.rules"), "prefix_rule(")?;

    let cases = [
        ("unknown", Vec::<(&Path, TrustLevel)>::new(), false),
        (
            "untrusted",
            vec![(&project_root as &Path, TrustLevel::Untrusted)],
            false,
        ),
        (
            "trusted",
            vec![(&project_root as &Path, TrustLevel::Trusted)],
            true,
        ),
    ];

    for (name, trust_entries, expect_warning) in cases {
        let codex_home = temp.path().join(format!("home_execpolicy_warning_{name}"));
        fs::create_dir_all(&codex_home)?;
        write_project_trust_config(&codex_home, &trust_entries).await?;

        let config = ConfigBuilder::default()
            .codex_home(codex_home)
            .fallback_cwd(Some(nested.clone()))
            .build()
            .await?;

        let warning = check_execpolicy_for_warnings(&config.config_layer_stack)
            .await
            .map_err(std::io::Error::other)?;
        assert_eq!(
            matches!(warning, Some(ExecPolicyError::ParsePolicy { .. })),
            expect_warning,
            "unexpected execpolicy warning state for {name}",
        );
    }

    Ok(())
}
