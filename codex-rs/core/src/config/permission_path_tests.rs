//! Permission compilation preserves executor paths and native policy semantics.

use super::super::permissions::CompiledPermissionProfile;
use super::super::permissions::WorkspaceWriteSettings;
use super::super::permissions::compile_permission_profile;
use super::relative_subpath;
use codex_config::ConfigPathContext;
use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_config::permissions_toml::FilesystemPermissionToml::Access;
use codex_config::permissions_toml::FilesystemPermissionsToml;
use codex_config::permissions_toml::PermissionProfileToml;
use codex_config::permissions_toml::PermissionsToml;
use codex_config::permissions_toml::WorkspaceRootsToml;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemAccessMode::Deny;
use codex_protocol::permissions::FileSystemAccessMode::Read;
use codex_protocol::permissions::FileSystemAccessMode::Write;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::permissions::ReadDenyMatcher;
use codex_protocol::permissions::project_roots_glob_pattern;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::path::Path;

struct ExecutorPathFacts {
    base_dir: PathUri,
    user_home_dir: Option<PathUri>,
}

impl ExecutorPathFacts {
    fn context(&self) -> ConfigPathContext {
        ConfigPathContext::new(
            self.base_dir.infer_path_convention().unwrap(),
            Some(self.base_dir.clone()),
            self.user_home_dir.clone(),
        )
    }

    fn policy_context(&self) -> FileSystemSandboxPolicyContext<'_> {
        FileSystemSandboxPolicyContext {
            cwd: &self.base_dir,
            workspace_roots: std::slice::from_ref(&self.base_dir),
            user_home_dir: self.user_home_dir.as_ref(),
            temporary_directories: None,
        }
    }

    fn compile(&self, permissions: &PermissionsToml) -> std::io::Result<CompiledPermissionProfile> {
        compile_permission_profile(
            Some(permissions),
            "remote",
            &self.context(),
            /*workspace_write*/ None,
            &mut Vec::new(),
        )
    }
}

fn executor_contexts() -> impl Iterator<Item = ExecutorPathFacts> {
    [
        ("file:///workspace/project", "file:///home/executor"),
        ("file:///Users/executor/project", "file:///Users/executor"),
        ("file:///C:/workspace/project", "file:///C:/Users/executor"),
        (
            "file://server/share/project",
            "file://server/share/users/executor",
        ),
    ]
    .into_iter()
    .map(|(cwd, home)| ExecutorPathFacts {
        base_dir: PathUri::parse(cwd).unwrap(),
        user_home_dir: Some(PathUri::parse(home).unwrap()),
    })
}

fn permissions_with_rule(
    path: impl Into<String>,
    permission: FilesystemPermissionToml,
) -> PermissionsToml {
    PermissionsToml {
        entries: BTreeMap::from([(
            "remote".to_string(),
            PermissionProfileToml {
                filesystem: Some(FilesystemPermissionsToml {
                    glob_scan_max_depth: None,
                    entries: BTreeMap::from([(path.into(), permission)]),
                }),
                ..Default::default()
            },
        )]),
    }
}

fn scoped(entries: &[(&str, FileSystemAccessMode)]) -> FilesystemPermissionToml {
    FilesystemPermissionToml::Scoped(
        entries
            .iter()
            .map(|(path, access)| (path.to_string(), *access))
            .collect(),
    )
}

#[track_caller]
fn assert_invalid(result: std::io::Result<CompiledPermissionProfile>) {
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}

#[track_caller]
fn assert_read_denials(
    policy: &FileSystemSandboxPolicy,
    context: &FileSystemSandboxPolicyContext<'_>,
    cases: &[(PathUri, bool)],
) {
    let matcher = ReadDenyMatcher::try_new_with_context(policy, context)
        .unwrap()
        .unwrap();
    for (path, denied) in cases {
        assert_eq!(
            matcher.is_read_denied_uri(path, context),
            *denied,
            "path={path}"
        );
    }
}

#[track_caller]
fn assert_executor_rule(
    permissions: &PermissionsToml,
    context: &ConfigPathContext,
    expected: FileSystemSandboxEntry,
) -> CompiledPermissionProfile {
    let mut warnings = Vec::new();
    let compiled = compile_permission_profile(
        Some(permissions),
        "remote",
        context,
        /*workspace_write*/ None,
        &mut warnings,
    )
    .unwrap();
    assert_eq!(
        (
            compiled.permission_profile.to_runtime_permissions(),
            &compiled.workspace_roots
        ),
        (
            (
                FileSystemSandboxPolicy::restricted(vec![expected]),
                NetworkSandboxPolicy::Restricted
            ),
            &Vec::<PathUri>::new(),
        ),
        "permissions={permissions:?}, context={context:?}",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    compiled
}

fn deny_glob_entry(pattern: impl Into<String>) -> FileSystemSandboxEntry {
    FileSystemSandboxEntry::new(
        FileSystemPath::GlobPattern {
            pattern: pattern.into(),
        },
        Deny,
    )
}

#[test]
fn compiler_uses_shared_resolution_for_literal_and_glob_rules() {
    for facts in executor_contexts() {
        let home = facts.user_home_dir.as_ref().unwrap();
        let root = home.join("shared").unwrap();
        let literal = FileSystemSandboxEntry::new(root.into(), Write);
        let glob = deny_glob_entry(
            home.join("shared/*.env")
                .unwrap()
                .inferred_native_path_string(),
        );
        for (input, permission, expected) in [
            ("~/shared/**".into(), Access(Write), literal.clone()),
            ("~/shared/*.env".into(), Access(Deny), glob.clone()),
            (
                home.inferred_native_path_string(),
                scoped(&[("shared", Write)]),
                literal,
            ),
            (
                home.inferred_native_path_string(),
                scoped(&[("shared/*.env", Deny)]),
                glob,
            ),
        ] {
            assert_executor_rule(
                &permissions_with_rule(input, permission),
                &facts.context(),
                expected,
            );
        }
    }
}

#[test]
fn scoped_home_rules_resolve_and_deny_the_supplied_home() {
    for facts in executor_contexts() {
        let home = facts.user_home_dir.as_ref().unwrap();
        let home_glob = match facts.context().convention() {
            PathConvention::Posix => "~/private/*.env",
            PathConvention::Windows => r"~\private\*.env",
        };
        for outer in [
            facts.base_dir.inferred_native_path_string(),
            ":workspace_roots".into(),
            ":project_roots".into(),
        ] {
            for (subpath, resolved) in [
                ("~", home.clone()),
                ("~/private", home.join("private").unwrap()),
                (home_glob, home.join("private/*.env").unwrap()),
            ] {
                let expected = if subpath.contains('*') {
                    deny_glob_entry(resolved.inferred_native_path_string())
                } else {
                    FileSystemSandboxEntry::new(resolved.into(), Deny)
                };
                let permissions = permissions_with_rule(&outer, scoped(&[(subpath, Deny)]));
                let compiled = assert_executor_rule(&permissions, &facts.context(), expected);
                assert_read_denials(
                    &compiled.permission_profile.file_system_sandbox_policy(),
                    &facts.policy_context(),
                    &[
                        (home.join("private/key.env").unwrap(), true),
                        (
                            facts.base_dir.join("private/key.env").unwrap(),
                            subpath == "~" && facts.base_dir.starts_with(home),
                        ),
                    ],
                );
            }
            for subpath in ["~/../private", "~/child/../private"] {
                let permissions = permissions_with_rule(&outer, scoped(&[(subpath, Deny)]));
                assert_invalid(facts.compile(&permissions));
            }
        }
    }
}

#[test]
fn inherited_workspace_roots_apply_to_executor_policy_and_read_denials() {
    let permissions: PermissionsToml = toml::from_str(
        r#"
        [base]
        extends = ":read-only"
        [base.workspace_roots]
        "../shared" = true
        "~/scratch" = true
        [remote]
        extends = "base"
        [remote.workspace_roots]
        "../shared" = false
        "./profile-root" = true
        [remote.filesystem]
        glob_scan_max_depth = 3
        [remote.filesystem.":workspace_roots"]
        "." = "write"
        "private/*.env" = "deny"
        [remote.network]
        enabled = true
    "#,
    )
    .unwrap();
    for facts in executor_contexts() {
        let compiled = facts.compile(&permissions).unwrap();
        let (policy, network) = compiled.permission_profile.to_runtime_permissions();
        let roots = compiled.workspace_roots;
        let home = facts.user_home_dir.as_ref().unwrap();
        assert_eq!(
            (&roots, network),
            (
                &vec![
                    facts.base_dir.join("profile-root").unwrap(),
                    home.join("scratch").unwrap()
                ],
                NetworkSandboxPolicy::Enabled
            ),
        );
        let policy =
            policy.materialize_project_roots_with_path_uris(std::slice::from_ref(&facts.base_dir));
        let context = facts.policy_context();
        for root in std::iter::once(&facts.base_dir).chain(&roots) {
            assert!(policy.can_write_path(&root.join("output").unwrap(), &context));
            assert_read_denials(
                &policy,
                &context,
                &[
                    (root.join("private/key.env").unwrap(), true),
                    (root.join("private/key.txt").unwrap(), false),
                ],
            );
        }
    }
}

#[test]
fn interior_dot_workspace_glob_fails_closed_for_every_path_convention() {
    let permissions = permissions_with_rule(
        ":workspace_roots",
        scoped(&[(".", Write), ("private/./*.env", Deny)]),
    );
    // Preserve the symbolic spelling; strict materialization rejects dot components.
    let expected = (
        FileSystemSandboxPolicy::restricted(vec![
            FileSystemSandboxEntry::new(
                FileSystemPath::Special {
                    value: codex_protocol::permissions::FileSystemSpecialPath::project_roots(
                        /*subpath*/ None,
                    ),
                },
                Write,
            ),
            deny_glob_entry("codex-project-roots://private/./*.env"),
        ]),
        NetworkSandboxPolicy::Restricted,
    );
    for facts in executor_contexts() {
        let compiled = facts.compile(&permissions).unwrap();
        assert_eq!(
            compiled.permission_profile.to_runtime_permissions(),
            expected
        );
        let policy = compiled
            .permission_profile
            .file_system_sandbox_policy()
            .materialize_project_roots_with_path_uris(std::slice::from_ref(&facts.base_dir));
        let context = facts.policy_context();
        assert!(!policy.can_write_path(&facts.base_dir.join("output").unwrap(), &context));
        assert_read_denials(
            &policy,
            &context,
            &[
                (facts.base_dir.join("private/key.env").unwrap(), true),
                (facts.base_dir.join("private/key.txt").unwrap(), true),
            ],
        );
    }
}

#[test]
fn permission_rules_require_absolute_paths_and_descendant_subpaths() {
    for mut facts in executor_contexts() {
        facts.user_home_dir = None;
        // Unlike the shared resolver, permission rules cannot use cwd-relative paths.
        // Missing executor home must not fall back to this host's home.
        for input in [
            "relative",
            "./relative",
            "~",
            "~/private",
            "C:private",
            r"\private",
            // Preserve raw-text validation even when URI parsing would remove `a:b`.
            r"C:\base\a:b\..\private",
        ] {
            assert_invalid(facts.compile(&permissions_with_rule(input, Access(Read))));
        }
        for outer in [
            facts.base_dir.inferred_native_path_string(),
            ":workspace_roots".into(),
        ] {
            for input in ["../private", "./private", "child/../private"] {
                assert_invalid(
                    facts.compile(&permissions_with_rule(&outer, scoped(&[(input, Deny)]))),
                );
            }
        }
    }
}

#[test]
fn executor_scoped_globs_preserve_native_separator_spelling() {
    for facts in
        executor_contexts().filter(|facts| facts.context().convention() == PathConvention::Windows)
    {
        for subpath in ["secrets/*.env", r"secrets\*.env", r"nested/secrets\*.env"] {
            assert_executor_rule(
                &permissions_with_rule(":workspace_roots", scoped(&[(subpath, Deny)])),
                &facts.context(),
                deny_glob_entry(project_roots_glob_pattern(Path::new(subpath))),
            );
        }
    }
}

fn native_context() -> ConfigPathContext {
    ConfigPathContext::new(
        PathConvention::native(),
        std::env::current_dir()
            .ok()
            .and_then(|cwd| PathUri::from_host_native_path(cwd).ok()),
        AbsolutePathBufGuard::home_directory()
            .and_then(|home| PathUri::from_host_native_path(home).ok()),
    )
}

#[test]
fn native_path_grammar_preserves_component_validation_and_spelling() {
    let context = native_context();
    for input in [
        "",
        ".",
        "..",
        "./child",
        "../child",
        "child/../other",
        "child/./other",
        "child//other/",
        "~/child",
        r"~\child",
        r"C:child",
        "/child",
        r"\child",
        "child/.../leaf",
    ] {
        let native_accepts = !input.is_empty()
            && Path::new(input)
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)));
        match relative_subpath(input, &context) {
            Ok(actual) => {
                assert!(native_accepts, "{input}");
                assert_eq!(actual, input);
            }
            Err(_) => assert!(!native_accepts, "{input}"),
        }
    }
}

#[cfg(unix)]
#[test]
fn non_unicode_home_is_rejected_by_the_shared_compiler() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let home = std::path::PathBuf::from(OsString::from_vec(b"/home/native-\xff".to_vec()));
    AbsolutePathBufGuard::with_home_directory(&home, || {
        let permissions = permissions_with_rule("~/read", Access(Read));
        assert_invalid(compile_permission_profile(
            Some(&permissions),
            "remote",
            &native_context(),
            /*workspace_write*/ None,
            &mut Vec::new(),
        ));
    });
}

#[test]
fn deny_globs_validate_literal_home_and_scoped_directories() {
    for mut facts in executor_contexts() {
        let directory = facts.base_dir.join("sam[1]").unwrap();
        facts.user_home_dir = Some(directory.clone());
        for (outer, permission) in [
            ("~/private/*.key".into(), Access(Deny)),
            ("~/private".into(), scoped(&[("*.key", Deny)])),
            (
                directory.inferred_native_path_string(),
                scoped(&[("private/*.key", Deny)]),
            ),
        ] {
            assert_invalid(facts.compile(&permissions_with_rule(outer, permission)));
        }
    }
}

#[test]
fn compiler_applies_deny_globs_safely_to_literal_profile_workspace_roots() {
    for facts in executor_contexts() {
        let root = facts.base_dir.join("sam[1]").unwrap();
        let mut permissions = permissions_with_rule(
            ":workspace_roots",
            scoped(&[(".", Write), ("private/*.key", Deny)]),
        );
        permissions
            .entries
            .get_mut("remote")
            .unwrap()
            .workspace_roots = Some(WorkspaceRootsToml {
            entries: BTreeMap::from([(root.inferred_native_path_string(), true)]),
        });
        let compiled = facts.compile(&permissions).unwrap();
        assert_eq!(compiled.workspace_roots, vec![root.clone()]);
        // Infallible materialization conservatively denies the literal root.
        assert_read_denials(
            &compiled.permission_profile.file_system_sandbox_policy(),
            &facts.policy_context(),
            &[
                (root.join("private/a.key").unwrap(), true),
                (root.join("public.txt").unwrap(), true),
                (facts.base_dir.join("sam1/private/a.key").unwrap(), false),
            ],
        );
    }
}

#[test]
fn builtin_workspace_settings_use_executor_roots_in_the_compiled_profile() {
    for facts in executor_contexts() {
        let root = facts.base_dir.join("legacy-root").unwrap();
        let settings = WorkspaceWriteSettings {
            writable_roots: vec![root.clone(), root.clone()],
            network_access: true,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        };
        let compiled = compile_permission_profile(
            /*permissions*/ None,
            ":workspace",
            &facts.context(),
            Some(&settings),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(compiled.workspace_roots, vec![root.clone()]);
        assert_eq!(
            compiled.permission_profile.network_sandbox_policy(),
            NetworkSandboxPolicy::Enabled,
        );
        let policy_context = facts.policy_context();
        assert!(
            compiled
                .permission_profile
                .file_system_sandbox_policy()
                .can_write_path(&root.join("output").unwrap(), &policy_context)
        );
    }
}
