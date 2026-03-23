use super::CoreShellActionProvider;
#[cfg(target_os = "macos")]
use super::CoreShellCommandExecutor;
use super::InterceptedExecPolicyContext;
use super::ParsedShellCommand;
use super::commands_for_intercepted_exec_policy;
use super::evaluate_intercepted_exec_policy;
use super::extract_shell_script;
use super::is_unconfigured_zsh_exec;
use super::join_program_and_argv;
use super::map_exec_result;
use super::resolve_host_zsh_paths;
use super::resolve_host_zsh_paths_from_path_env;
#[cfg(target_os = "macos")]
use crate::config::Constrained;
#[cfg(target_os = "macos")]
use crate::config::Permissions;
#[cfg(target_os = "macos")]
use crate::config::types::ShellEnvironmentPolicy;
use crate::exec::SandboxType;
use crate::protocol::AskForApproval;
use crate::protocol::GranularApprovalConfig;
use crate::protocol::ReadOnlyAccess;
use crate::protocol::SandboxPolicy;
use crate::sandboxing::SandboxPermissions;
#[cfg(target_os = "macos")]
use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use crate::shell::Shell;
use crate::shell::ShellType;
use crate::shell_snapshot::ShellSnapshot;
use crate::skills::SkillMetadata;
use codex_execpolicy::Decision;
use codex_execpolicy::Evaluation;
use codex_execpolicy::PolicyParser;
use codex_execpolicy::RuleMatch;
#[cfg(target_os = "macos")]
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::MacOsPreferencesPermission;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SkillScope;
use codex_shell_command::parse_command::shlex_join;
use codex_shell_escalation::EscalationExecution;
use codex_shell_escalation::EscalationPermissions;
use codex_shell_escalation::ExecResult;
use codex_shell_escalation::Permissions as EscalatedPermissions;
#[cfg(target_os = "macos")]
use codex_shell_escalation::ShellCommandExecutor;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::watch;

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

fn starlark_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn read_only_file_system_sandbox_policy() -> FileSystemSandboxPolicy {
    FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
        path: FileSystemPath::Special {
            value: FileSystemSpecialPath::Root,
        },
        access: FileSystemAccessMode::Read,
    }])
}

#[cfg(target_os = "macos")]
fn unrestricted_file_system_sandbox_policy() -> FileSystemSandboxPolicy {
    FileSystemSandboxPolicy::unrestricted()
}

fn test_skill_metadata(permission_profile: Option<PermissionProfile>) -> SkillMetadata {
    SkillMetadata {
        name: "skill".to_string(),
        description: "description".to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        permission_profile,
        managed_network_override: None,
        path_to_skills_md: PathBuf::from("/tmp/skill/SKILL.md"),
        scope: SkillScope::User,
    }
}

fn shell_with_snapshot(
    shell_type: ShellType,
    shell_path: &str,
    snapshot_path: PathBuf,
    snapshot_cwd: PathBuf,
) -> Shell {
    let (_tx, shell_snapshot) = watch::channel(Some(Arc::new(ShellSnapshot {
        path: snapshot_path,
        cwd: snapshot_cwd,
    })));
    Shell {
        shell_type,
        shell_path: PathBuf::from(shell_path),
        shell_snapshot,
    }
}

#[test]
fn execve_prompt_rejection_uses_skill_approval_for_skill_scripts() {
    let decision_source = super::DecisionSource::SkillScript {
        skill: test_skill_metadata(None),
    };

    assert_eq!(
        super::execve_prompt_is_rejected_by_policy(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            &decision_source,
        ),
        None,
    );
    assert_eq!(
        super::execve_prompt_is_rejected_by_policy(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: false,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            &decision_source,
        ),
        Some("approval required by skill, but AskForApproval::Granular.skill_approval is false"),
    );
}

#[test]
fn execve_prompt_rejection_keeps_prefix_rules_on_rules_flag() {
    assert_eq!(
        super::execve_prompt_is_rejected_by_policy(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: false,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            &super::DecisionSource::PrefixRule,
        ),
        Some("approval required by policy rule, but AskForApproval::Granular.rules is false"),
    );
}

#[test]
fn execve_prompt_rejection_keeps_unmatched_commands_on_sandbox_flag() {
    assert_eq!(
        super::execve_prompt_is_rejected_by_policy(
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: false,
                rules: true,
                skill_approval: true,
                request_permissions: true,
                mcp_elicitations: true,
            }),
            &super::DecisionSource::UnmatchedCommandFallback,
        ),
        Some("approval required by policy, but AskForApproval::Granular.sandbox_approval is false"),
    );
}

#[test]
fn approval_sandbox_permissions_only_downgrades_preapproved_additional_permissions() {
    assert_eq!(
        super::approval_sandbox_permissions(SandboxPermissions::WithAdditionalPermissions, true),
        SandboxPermissions::UseDefault,
    );
    assert_eq!(
        super::approval_sandbox_permissions(SandboxPermissions::WithAdditionalPermissions, false),
        SandboxPermissions::WithAdditionalPermissions,
    );
    assert_eq!(
        super::approval_sandbox_permissions(SandboxPermissions::RequireEscalated, true),
        SandboxPermissions::RequireEscalated,
    );
}

#[test]
fn build_exact_match_zsh_fork_reexec_sources_snapshot_inside_inner_zsh_and_drops_outer_login() {
    let dir = tempdir().expect("create temp dir");
    let snapshot_path = dir.path().join("snapshot.sh");
    std::fs::write(
        &snapshot_path,
        "# Snapshot file\nexport SNAPSHOT_LOGIN_ENV='from_snapshot'\n",
    )
    .expect("write snapshot");
    let session_shell = shell_with_snapshot(
        ShellType::Zsh,
        "/bin/zsh",
        snapshot_path.clone(),
        dir.path().to_path_buf(),
    );
    let shell_command = ParsedShellCommand {
        program: "/bin/zsh".to_string(),
        script: "python3 -c 'print(42)'".to_string(),
        login: true,
    };

    let explicit_env_overrides = HashMap::from([("PATH".to_string(), "/worktree/bin".to_string())]);
    let reexec = super::build_exact_match_zsh_fork_reexec(
        &shell_command,
        &session_shell,
        dir.path(),
        &explicit_env_overrides,
        Path::new("/opt/codex/zsh"),
    );

    assert_eq!(reexec.exact_intercepted_command[0], "/opt/codex/zsh");
    assert_eq!(reexec.exact_intercepted_command[1], "-c");
    assert!(reexec.exact_intercepted_command[2].contains(&format!(
        "if . '{}' >/dev/null 2>&1; then :; fi",
        snapshot_path.display()
    )));
    assert!(
        reexec.exact_intercepted_command[2]
            .contains("export PATH=\"${__CODEX_SNAPSHOT_OVERRIDE_0}\"")
    );
    assert!(
        reexec.exact_intercepted_command[2].ends_with("python3 -c 'print(42)'"),
        "expected inner command to end with original script, got {:?}",
        reexec.exact_intercepted_command[2]
    );
    assert_eq!(
        reexec.outer_command,
        vec![
            "/bin/zsh".to_string(),
            "-c".to_string(),
            format!("exec {}", shlex_join(&reexec.exact_intercepted_command)),
        ]
    );
}

#[test]
fn build_exact_match_zsh_fork_reexec_preserves_outer_login_without_snapshot() {
    let shell_command = ParsedShellCommand {
        program: "/bin/zsh".to_string(),
        script: "python3 -c 'print(42)'".to_string(),
        login: true,
    };
    let session_shell = Shell {
        shell_type: ShellType::Zsh,
        shell_path: PathBuf::from("/bin/zsh"),
        shell_snapshot: crate::shell::empty_shell_snapshot_receiver(),
    };

    let reexec = super::build_exact_match_zsh_fork_reexec(
        &shell_command,
        &session_shell,
        Path::new("/tmp"),
        &HashMap::new(),
        Path::new("/opt/codex/zsh"),
    );
    let expected_inner = vec![
        "/opt/codex/zsh".to_string(),
        "-c".to_string(),
        "python3 -c 'print(42)'".to_string(),
    ];

    assert_eq!(reexec.exact_intercepted_command, expected_inner);
    assert_eq!(
        reexec.outer_command,
        vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            format!("exec {}", shlex_join(&expected_inner)),
        ]
    );
}

#[test]
fn extract_shell_script_preserves_login_flag() {
    assert_eq!(
        extract_shell_script(&["/bin/zsh".into(), "-lc".into(), "echo hi".into()]).unwrap(),
        ParsedShellCommand {
            program: "/bin/zsh".to_string(),
            script: "echo hi".to_string(),
            login: true,
        }
    );
    assert_eq!(
        extract_shell_script(&["/bin/zsh".into(), "-c".into(), "echo hi".into()]).unwrap(),
        ParsedShellCommand {
            program: "/bin/zsh".to_string(),
            script: "echo hi".to_string(),
            login: false,
        }
    );
}

#[test]
fn extract_shell_script_rejects_wrapped_command_prefixes() {
    let err = extract_shell_script(&[
        "/usr/bin/env".into(),
        "CODEX_EXECVE_WRAPPER=1".into(),
        "/bin/zsh".into(),
        "-lc".into(),
        "echo hello".into(),
    ])
    .unwrap_err();
    assert!(matches!(err, super::ToolError::Rejected(_)));
}

#[test]
fn extract_shell_script_rejects_unsupported_shell_invocation() {
    let err = extract_shell_script(&[
        "sandbox-exec".into(),
        "-fc".into(),
        "echo not supported".into(),
    ])
    .unwrap_err();
    assert!(matches!(err, super::ToolError::Rejected(_)));
    assert_eq!(
        match err {
            super::ToolError::Rejected(reason) => reason,
            _ => "".to_string(),
        },
        "unexpected shell command format for zsh-fork execution"
    );
}

#[test]
fn join_program_and_argv_replaces_original_argv_zero() {
    assert_eq!(
        join_program_and_argv(
            &AbsolutePathBuf::from_absolute_path("/tmp/tool").unwrap(),
            &["./tool".into(), "--flag".into(), "value".into()],
        ),
        vec!["/tmp/tool", "--flag", "value"]
    );
    assert_eq!(
        join_program_and_argv(
            &AbsolutePathBuf::from_absolute_path("/tmp/tool").unwrap(),
            &["./tool".into()]
        ),
        vec!["/tmp/tool"]
    );
}

#[test]
fn is_unconfigured_zsh_exec_matches_non_configured_zsh_paths() {
    let program = AbsolutePathBuf::try_from(host_absolute_path(&["bin", "zsh"])).unwrap();
    let host = vec![PathBuf::from(host_absolute_path(&["bin", "zsh"]))];
    let configured = PathBuf::from(host_absolute_path(&["tmp", "codex-zsh"]));
    assert!(is_unconfigured_zsh_exec(
        &program,
        Some(configured.as_path()),
        &host,
    ));
}

#[test]
fn is_unconfigured_zsh_exec_ignores_non_zsh_or_configured_paths() {
    let configured = PathBuf::from(host_absolute_path(&["tmp", "codex-zsh"]));
    let host = vec![PathBuf::from(host_absolute_path(&["bin", "zsh"]))];
    let configured_program = AbsolutePathBuf::try_from(configured.clone()).unwrap();
    assert!(!is_unconfigured_zsh_exec(
        &configured_program,
        Some(configured.as_path()),
        &host,
    ));

    let non_zsh =
        AbsolutePathBuf::try_from(host_absolute_path(&["usr", "bin", "python3"])).unwrap();
    assert!(!is_unconfigured_zsh_exec(
        &non_zsh,
        Some(configured.as_path()),
        &host,
    ));
    assert!(!is_unconfigured_zsh_exec(&non_zsh, None, &host,));
}

#[test]
fn is_unconfigured_zsh_exec_does_not_match_non_host_zsh_named_binaries() {
    let program = AbsolutePathBuf::try_from(host_absolute_path(&["tmp", "repo", "zsh"])).unwrap();
    let configured = PathBuf::from(host_absolute_path(&["tmp", "codex-zsh"]));
    let host = vec![PathBuf::from(host_absolute_path(&["bin", "zsh"]))];
    assert!(!is_unconfigured_zsh_exec(
        &program,
        Some(configured.as_path()),
        &host,
    ));
}

#[test]
fn is_unconfigured_zsh_exec_matches_any_approved_host_zsh_path() {
    let program =
        AbsolutePathBuf::try_from(host_absolute_path(&["usr", "local", "bin", "zsh"])).unwrap();
    let configured = PathBuf::from(host_absolute_path(&["tmp", "codex-zsh"]));
    let host_paths = vec![
        PathBuf::from(host_absolute_path(&["bin", "zsh"])),
        PathBuf::from(host_absolute_path(&["usr", "local", "bin", "zsh"])),
    ];
    assert!(is_unconfigured_zsh_exec(
        &program,
        Some(configured.as_path()),
        &host_paths,
    ));
}

#[test]
fn resolve_host_zsh_paths_ignores_repo_local_path_shadowing() {
    let shadow_dir = tempfile::tempdir().expect("create shadow dir");
    let cwd_dir = tempfile::tempdir().expect("create cwd dir");
    let fake_zsh = shadow_dir.path().join("zsh");
    std::fs::write(&fake_zsh, "#!/bin/sh\nexit 0\n").expect("write fake zsh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&fake_zsh)
            .expect("metadata for fake zsh")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_zsh, permissions).expect("chmod fake zsh");
    }

    let path_env =
        std::env::join_paths([shadow_dir.path(), Path::new("/usr/bin"), Path::new("/bin")])
            .expect("join PATH")
            .into_string()
            .expect("PATH should be UTF-8");
    let resolved = resolve_host_zsh_paths(Some(&path_env), cwd_dir.path());
    assert!(
        resolved.iter().all(|path| path != fake_zsh.as_path()),
        "repo-local shadow zsh should not be treated as a host shell: {resolved:?}"
    );
}

#[test]
fn resolve_host_zsh_paths_accept_registered_nonstandard_path_entries() {
    let host_dir = tempfile::tempdir().expect("create host dir");
    let second_host_dir = tempfile::tempdir().expect("create second host dir");
    let host_zsh = host_dir.path().join("zsh");
    let second_host_zsh = second_host_dir.path().join("zsh");
    std::fs::write(&host_zsh, "#!/bin/sh\nexit 0\n").expect("write host zsh");
    std::fs::write(&second_host_zsh, "#!/bin/sh\nexit 0\n").expect("write second host zsh");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for host_zsh in [&host_zsh, &second_host_zsh] {
            let mut permissions = std::fs::metadata(host_zsh)
                .expect("metadata for host zsh")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(host_zsh, permissions).expect("chmod host zsh");
        }
    }

    let path_env = std::env::join_paths([
        host_dir.path(),
        second_host_dir.path(),
        Path::new("/usr/bin"),
        Path::new("/bin"),
    ])
    .expect("join PATH")
    .into_string()
    .expect("PATH should be UTF-8");
    let expected = vec![
        host_zsh
            .canonicalize()
            .expect("host zsh path should canonicalize"),
        second_host_zsh
            .canonicalize()
            .expect("second host zsh path should canonicalize"),
    ];
    let resolved = resolve_host_zsh_paths_from_path_env(Some(&path_env), expected.as_slice());
    assert_eq!(resolved, expected);
}

#[test]
fn commands_for_intercepted_exec_policy_parses_plain_shell_wrappers() {
    let program = AbsolutePathBuf::try_from(host_absolute_path(&["bin", "bash"])).unwrap();
    let candidate_commands = commands_for_intercepted_exec_policy(
        &program,
        &["not-bash".into(), "-lc".into(), "git status && pwd".into()],
    );

    assert_eq!(
        candidate_commands.commands,
        vec![
            vec!["git".to_string(), "status".to_string()],
            vec!["pwd".to_string()],
        ]
    );
    assert!(!candidate_commands.used_complex_parsing);
}

#[test]
fn map_exec_result_preserves_stdout_and_stderr() {
    let out = map_exec_result(
        SandboxType::None,
        ExecResult {
            exit_code: 0,
            stdout: "out".to_string(),
            stderr: "err".to_string(),
            output: "outerr".to_string(),
            duration: Duration::from_millis(1),
            timed_out: false,
        },
    )
    .unwrap();

    assert_eq!(out.stdout.text, "out");
    assert_eq!(out.stderr.text, "err");
    assert_eq!(out.aggregated_output.text, "outerr");
}

#[test]
fn shell_request_escalation_execution_is_explicit() {
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: None,
            write: Some(vec![
                AbsolutePathBuf::from_absolute_path("/tmp/output").unwrap(),
            ]),
        }),
        ..Default::default()
    };
    let sandbox_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![AbsolutePathBuf::from_absolute_path("/tmp/original/output").unwrap()],
        read_only_access: ReadOnlyAccess::FullAccess,
        network_access: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };
    let file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::from_absolute_path("/tmp/original/output").unwrap(),
            },
            access: FileSystemAccessMode::Write,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::from_absolute_path("/tmp/secret").unwrap(),
            },
            access: FileSystemAccessMode::None,
        },
    ]);
    let network_sandbox_policy = NetworkSandboxPolicy::Restricted;
    let macos_seatbelt_profile_extensions = MacOsSeatbeltProfileExtensions {
        macos_preferences: MacOsPreferencesPermission::ReadWrite,
        ..Default::default()
    };

    assert_eq!(
        CoreShellActionProvider::shell_request_escalation_execution(
            crate::sandboxing::SandboxPermissions::UseDefault,
            &sandbox_policy,
            &file_system_sandbox_policy,
            network_sandbox_policy,
            None,
            Some(&macos_seatbelt_profile_extensions),
        ),
        EscalationExecution::TurnDefault,
    );
    assert_eq!(
        CoreShellActionProvider::shell_request_escalation_execution(
            crate::sandboxing::SandboxPermissions::RequireEscalated,
            &sandbox_policy,
            &file_system_sandbox_policy,
            network_sandbox_policy,
            None,
            Some(&macos_seatbelt_profile_extensions),
        ),
        EscalationExecution::Unsandboxed,
    );
    assert_eq!(
        CoreShellActionProvider::shell_request_escalation_execution(
            crate::sandboxing::SandboxPermissions::WithAdditionalPermissions,
            &sandbox_policy,
            &file_system_sandbox_policy,
            network_sandbox_policy,
            Some(&requested_permissions),
            Some(&macos_seatbelt_profile_extensions),
        ),
        EscalationExecution::Permissions(EscalationPermissions::Permissions(
            EscalatedPermissions {
                sandbox_policy,
                file_system_sandbox_policy,
                network_sandbox_policy,
                macos_seatbelt_profile_extensions: Some(macos_seatbelt_profile_extensions),
            },
        )),
    );
}

#[test]
fn skill_escalation_execution_uses_additional_permissions() {
    let requested_permissions = PermissionProfile {
        file_system: Some(FileSystemPermissions {
            read: None,
            write: Some(vec![
                AbsolutePathBuf::from_absolute_path("/tmp/output").unwrap(),
            ]),
        }),
        ..Default::default()
    };

    assert_eq!(
        CoreShellActionProvider::skill_escalation_execution(&test_skill_metadata(Some(
            requested_permissions.clone(),
        ))),
        EscalationExecution::Permissions(EscalationPermissions::PermissionProfile(
            requested_permissions,
        )),
    );
}

#[test]
fn skill_escalation_execution_ignores_empty_permissions() {
    assert_eq!(
        CoreShellActionProvider::skill_escalation_execution(&test_skill_metadata(Some(
            PermissionProfile::default(),
        ))),
        EscalationExecution::TurnDefault,
    );
    assert_eq!(
        CoreShellActionProvider::skill_escalation_execution(&test_skill_metadata(None)),
        EscalationExecution::TurnDefault,
    );
}

#[test]
fn evaluate_intercepted_exec_policy_uses_wrapper_command_when_shell_wrapper_parsing_disabled() {
    let policy_src = r#"prefix_rule(pattern = ["npm", "publish"], decision = "prompt")"#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src).unwrap();
    let policy = parser.build();
    let program = AbsolutePathBuf::try_from(host_absolute_path(&["bin", "zsh"])).unwrap();

    let enable_intercepted_exec_policy_shell_wrapper_parsing = false;
    let evaluation = evaluate_intercepted_exec_policy(
        &policy,
        &program,
        &[
            "zsh".to_string(),
            "-lc".to_string(),
            "npm publish".to_string(),
        ],
        InterceptedExecPolicyContext {
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: &SandboxPolicy::new_read_only_policy(),
            file_system_sandbox_policy: &read_only_file_system_sandbox_policy(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            enable_shell_wrapper_parsing: enable_intercepted_exec_policy_shell_wrapper_parsing,
        },
    );

    assert!(
        matches!(
            evaluation.matched_rules.as_slice(),
            [RuleMatch::HeuristicsRuleMatch { command, decision: Decision::Allow }]
                if command == &vec![
                    program.to_string_lossy().to_string(),
                    "-lc".to_string(),
                    "npm publish".to_string(),
                ]
        ),
        r#"This is allowed because when shell wrapper parsing is disabled,
the policy evaluation does not try to parse the shell command and instead
matches the whole command line with the resolved program path, which in this
case is `/bin/zsh` followed by some arguments.

Because there is no policy rule for `/bin/zsh` or `zsh`, the decision is to
allow the command and let the sandbox be responsible for enforcing any
restrictions.

That said, if /bin/zsh is the zsh-fork, then the execve wrapper should
ultimately intercept the `npm publish` command and apply the policy rules to it.
"#
    );
}

#[test]
fn evaluate_intercepted_exec_policy_matches_inner_shell_commands_when_enabled() {
    let policy_src = r#"prefix_rule(pattern = ["npm", "publish"], decision = "prompt")"#;
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", policy_src).unwrap();
    let policy = parser.build();
    let program = AbsolutePathBuf::try_from(host_absolute_path(&["bin", "bash"])).unwrap();

    let enable_intercepted_exec_policy_shell_wrapper_parsing = true;
    let evaluation = evaluate_intercepted_exec_policy(
        &policy,
        &program,
        &[
            "bash".to_string(),
            "-lc".to_string(),
            "npm publish".to_string(),
        ],
        InterceptedExecPolicyContext {
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: &SandboxPolicy::new_read_only_policy(),
            file_system_sandbox_policy: &read_only_file_system_sandbox_policy(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            enable_shell_wrapper_parsing: enable_intercepted_exec_policy_shell_wrapper_parsing,
        },
    );

    assert_eq!(
        evaluation,
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: vec!["npm".to_string(), "publish".to_string()],
                decision: Decision::Prompt,
                resolved_program: None,
                justification: None,
            }],
        }
    );
}

#[test]
fn intercepted_exec_policy_uses_host_executable_mappings() {
    let git_path = host_absolute_path(&["usr", "bin", "git"]);
    let git_path_literal = starlark_string(&git_path);
    let policy_src = format!(
        r#"
prefix_rule(pattern = ["git", "status"], decision = "prompt")
host_executable(name = "git", paths = ["{git_path_literal}"])
"#
    );
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &policy_src).unwrap();
    let policy = parser.build();
    let program = AbsolutePathBuf::try_from(git_path).unwrap();

    let evaluation = evaluate_intercepted_exec_policy(
        &policy,
        &program,
        &["git".to_string(), "status".to_string()],
        InterceptedExecPolicyContext {
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: &SandboxPolicy::new_read_only_policy(),
            file_system_sandbox_policy: &read_only_file_system_sandbox_policy(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            enable_shell_wrapper_parsing: false,
        },
    );

    assert_eq!(
        evaluation,
        Evaluation {
            decision: Decision::Prompt,
            matched_rules: vec![RuleMatch::PrefixRuleMatch {
                matched_prefix: vec!["git".to_string(), "status".to_string()],
                decision: Decision::Prompt,
                resolved_program: Some(program),
                justification: None,
            }],
        }
    );
    assert!(CoreShellActionProvider::decision_driven_by_policy(
        &evaluation.matched_rules,
        evaluation.decision
    ));
}

#[test]
fn intercepted_exec_policy_treats_preapproved_additional_permissions_as_default() {
    let policy = PolicyParser::new().build();
    let program = AbsolutePathBuf::try_from(host_absolute_path(&["usr", "bin", "printf"])).unwrap();
    let argv = ["printf".to_string(), "hello".to_string()];
    let approval_policy = AskForApproval::OnRequest;
    let sandbox_policy = SandboxPolicy::new_workspace_write_policy();
    let file_system_sandbox_policy = read_only_file_system_sandbox_policy();

    let preapproved = evaluate_intercepted_exec_policy(
        &policy,
        &program,
        &argv,
        InterceptedExecPolicyContext {
            approval_policy,
            sandbox_policy: &sandbox_policy,
            file_system_sandbox_policy: &file_system_sandbox_policy,
            sandbox_permissions: super::approval_sandbox_permissions(
                SandboxPermissions::WithAdditionalPermissions,
                true,
            ),
            enable_shell_wrapper_parsing: false,
        },
    );
    let fresh_request = evaluate_intercepted_exec_policy(
        &policy,
        &program,
        &argv,
        InterceptedExecPolicyContext {
            approval_policy,
            sandbox_policy: &sandbox_policy,
            file_system_sandbox_policy: &file_system_sandbox_policy,
            sandbox_permissions: SandboxPermissions::WithAdditionalPermissions,
            enable_shell_wrapper_parsing: false,
        },
    );

    assert_eq!(preapproved.decision, Decision::Allow);
    assert_eq!(fresh_request.decision, Decision::Prompt);
}

#[test]
fn intercepted_exec_policy_rejects_disallowed_host_executable_mapping() {
    let allowed_git = host_absolute_path(&["usr", "bin", "git"]);
    let other_git = host_absolute_path(&["opt", "homebrew", "bin", "git"]);
    let allowed_git_literal = starlark_string(&allowed_git);
    let policy_src = format!(
        r#"
prefix_rule(pattern = ["git", "status"], decision = "prompt")
host_executable(name = "git", paths = ["{allowed_git_literal}"])
"#
    );
    let mut parser = PolicyParser::new();
    parser.parse("test.rules", &policy_src).unwrap();
    let policy = parser.build();
    let program = AbsolutePathBuf::try_from(other_git.clone()).unwrap();

    let evaluation = evaluate_intercepted_exec_policy(
        &policy,
        &program,
        &["git".to_string(), "status".to_string()],
        InterceptedExecPolicyContext {
            approval_policy: AskForApproval::OnRequest,
            sandbox_policy: &SandboxPolicy::new_read_only_policy(),
            file_system_sandbox_policy: &read_only_file_system_sandbox_policy(),
            sandbox_permissions: SandboxPermissions::UseDefault,
            enable_shell_wrapper_parsing: false,
        },
    );

    assert!(matches!(
        evaluation.matched_rules.as_slice(),
        [RuleMatch::HeuristicsRuleMatch { command, .. }]
            if command == &vec![other_git, "status".to_string()]
    ));
    assert!(!CoreShellActionProvider::decision_driven_by_policy(
        &evaluation.matched_rules,
        evaluation.decision
    ));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn prepare_escalated_exec_turn_default_preserves_macos_seatbelt_extensions() {
    let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).unwrap();
    let executor = CoreShellCommandExecutor {
        command: vec!["echo".to_string(), "ok".to_string()],
        cwd: cwd.to_path_buf(),
        env: HashMap::new(),
        network: None,
        sandbox: SandboxType::None,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        file_system_sandbox_policy: read_only_file_system_sandbox_policy(),
        network_sandbox_policy: NetworkSandboxPolicy::Restricted,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        sandbox_permissions: SandboxPermissions::UseDefault,
        justification: None,
        arg0: None,
        sandbox_policy_cwd: cwd.to_path_buf(),
        macos_seatbelt_profile_extensions: Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            ..Default::default()
        }),
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        shell_zsh_path: None,
        host_zsh_paths: Vec::new(),
    };

    let prepared = executor
        .prepare_escalated_exec(
            &AbsolutePathBuf::from_absolute_path("/bin/echo").unwrap(),
            &["echo".to_string(), "ok".to_string()],
            &cwd,
            HashMap::new(),
            EscalationExecution::TurnDefault,
        )
        .await
        .unwrap();

    assert_eq!(
        prepared.command.first().map(String::as_str),
        Some(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
    );
    assert_eq!(prepared.command.get(1).map(String::as_str), Some("-p"));
    assert!(
        prepared
            .command
            .get(2)
            .is_some_and(|policy| policy.contains("(allow user-preference-write)")),
        "expected seatbelt policy to include macOS extension profile: {:?}",
        prepared.command
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn prepare_escalated_exec_permissions_preserve_macos_seatbelt_extensions() {
    let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).unwrap();
    let executor = CoreShellCommandExecutor {
        command: vec!["echo".to_string(), "ok".to_string()],
        cwd: cwd.to_path_buf(),
        env: HashMap::new(),
        network: None,
        sandbox: SandboxType::None,
        sandbox_policy: SandboxPolicy::DangerFullAccess,
        file_system_sandbox_policy: unrestricted_file_system_sandbox_policy(),
        network_sandbox_policy: NetworkSandboxPolicy::Enabled,
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        sandbox_permissions: SandboxPermissions::UseDefault,
        justification: None,
        arg0: None,
        sandbox_policy_cwd: cwd.to_path_buf(),
        macos_seatbelt_profile_extensions: None,
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        shell_zsh_path: None,
        host_zsh_paths: Vec::new(),
    };

    let permissions = Permissions {
        approval_policy: Constrained::allow_any(AskForApproval::Never),
        sandbox_policy: Constrained::allow_any(SandboxPolicy::new_read_only_policy()),
        file_system_sandbox_policy: read_only_file_system_sandbox_policy(),
        network_sandbox_policy: codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
        network: None,
        allow_login_shell: true,
        shell_environment_policy: ShellEnvironmentPolicy::default(),
        windows_sandbox_mode: None,
        windows_sandbox_private_desktop: false,
        macos_seatbelt_profile_extensions: Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            ..Default::default()
        }),
    };

    let prepared = executor
        .prepare_escalated_exec(
            &AbsolutePathBuf::from_absolute_path("/bin/echo").unwrap(),
            &["echo".to_string(), "ok".to_string()],
            &cwd,
            HashMap::new(),
            EscalationExecution::Permissions(EscalationPermissions::Permissions(
                EscalatedPermissions {
                    sandbox_policy: permissions.sandbox_policy.get().clone(),
                    file_system_sandbox_policy: permissions.file_system_sandbox_policy.clone(),
                    network_sandbox_policy: permissions.network_sandbox_policy,
                    macos_seatbelt_profile_extensions: permissions
                        .macos_seatbelt_profile_extensions
                        .clone(),
                },
            )),
        )
        .await
        .unwrap();

    assert_eq!(
        prepared.command.first().map(String::as_str),
        Some(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
    );
    assert_eq!(prepared.command.get(1).map(String::as_str), Some("-p"));
    assert!(
        prepared
            .command
            .get(2)
            .is_some_and(|policy| policy.contains("(allow user-preference-write)")),
        "expected seatbelt policy to include macOS extension profile: {:?}",
        prepared.command
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn prepare_escalated_exec_permission_profile_unions_turn_and_requested_macos_extensions() {
    let cwd = AbsolutePathBuf::from_absolute_path(std::env::temp_dir()).unwrap();
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let executor = CoreShellCommandExecutor {
        command: vec!["echo".to_string(), "ok".to_string()],
        cwd: cwd.to_path_buf(),
        env: HashMap::new(),
        network: None,
        sandbox: SandboxType::None,
        sandbox_policy: sandbox_policy.clone(),
        file_system_sandbox_policy: read_only_file_system_sandbox_policy(),
        network_sandbox_policy: NetworkSandboxPolicy::from(&sandbox_policy),
        windows_sandbox_level: WindowsSandboxLevel::Disabled,
        windows_sandbox_private_desktop: false,
        sandbox_permissions: SandboxPermissions::UseDefault,
        justification: None,
        arg0: None,
        sandbox_policy_cwd: cwd.to_path_buf(),
        macos_seatbelt_profile_extensions: Some(MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadOnly,
            ..Default::default()
        }),
        codex_linux_sandbox_exe: None,
        use_legacy_landlock: false,
        shell_zsh_path: None,
        host_zsh_paths: Vec::new(),
    };

    let prepared = executor
        .prepare_escalated_exec(
            &AbsolutePathBuf::from_absolute_path("/bin/echo").unwrap(),
            &["echo".to_string(), "ok".to_string()],
            &cwd,
            HashMap::new(),
            EscalationExecution::Permissions(EscalationPermissions::PermissionProfile(
                PermissionProfile {
                    macos: Some(MacOsSeatbeltProfileExtensions {
                        macos_calendar: true,
                        macos_reminders: false,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
        )
        .await
        .unwrap();

    let policy = prepared
        .command
        .get(2)
        .expect("seatbelt policy should be present");
    assert_eq!(
        prepared.command.first().map(String::as_str),
        Some(MACOS_PATH_TO_SEATBELT_EXECUTABLE)
    );
    assert_eq!(prepared.command.get(1).map(String::as_str), Some("-p"));
    assert!(
        policy.contains("(allow user-preference-read)"),
        "expected turn macOS seatbelt extensions to be preserved: {:?}",
        prepared.command
    );
    assert!(
        policy.contains("(allow mach-lookup (global-name \"com.apple.CalendarAgent\"))"),
        "expected requested macOS seatbelt extensions to be included: {:?}",
        prepared.command
    );
}
