use super::*;
use crate::LoadedPlugin;
use crate::PluginMeasurementDefinition;
use crate::loader::curated_plugin_cache_version;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::startup_sync::curated_plugins_repo_path;
use crate::store::DEFAULT_PLUGIN_VERSION;
use crate::store::PluginStore;
use crate::test_support::TEST_CURATED_PLUGIN_SHA;
use crate::test_support::write_curated_plugin_sha_with;
use crate::test_support::write_openai_api_curated_marketplace;
use crate::test_support::write_openai_curated_marketplace;
use codex_plugin::PluginLoadOutcome;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::SkillDiscoveryMode;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;
const ENABLED: bool = true;
const DISABLED: bool = false;
fn path(path: &Path) -> AbsolutePathBuf {
    AbsolutePathBuf::from_absolute_path_checked(path).expect("absolute path")
}
fn loaded_plugin(config_name: &str, root: &Path, enabled: bool) -> LoadedPlugin {
    LoadedPlugin {
        config_name: config_name.to_string(),
        remote_plugin_id: None,
        manifest_name: None,
        plugin_namespace: None,
        manifest_description: None,
        root: path(root),
        enabled,
        skill_roots: Vec::new(),
        skill_discovery_mode: SkillDiscoveryMode::Recursive,
        disabled_skill_paths: HashSet::new(),
        has_enabled_skills: false,
        mcp_servers: HashMap::new(),
        apps: Vec::new(),
        hook_sources: Vec::new(),
        hook_load_warnings: Vec::new(),
        error: None,
    }
}
fn synced_plugin_root(codex_home: &Path, marketplace: &str, plugin_name: &str) -> AbsolutePathBuf {
    let synced_root = curated_plugins_repo_path(codex_home);
    match marketplace {
        OPENAI_CURATED_MARKETPLACE_NAME => {
            write_openai_curated_marketplace(&synced_root, &[plugin_name])
        }
        OPENAI_API_CURATED_MARKETPLACE_NAME => {
            write_openai_api_curated_marketplace(&synced_root, &[plugin_name])
        }
        _ => panic!("unsupported test marketplace"),
    }
    let plugin_id =
        PluginId::new(plugin_name.to_string(), marketplace.to_string()).expect("plugin id");
    let root = PluginStore::new(codex_home.to_path_buf()).plugin_root(
        &plugin_id,
        &curated_plugin_cache_version(TEST_CURATED_PLUGIN_SHA),
    );
    fs::create_dir_all(root.as_path()).expect("create cached plugin root");
    root
}
fn cached_remote_plugin_root(codex_home: &Path, plugin_name: &str) -> AbsolutePathBuf {
    let plugin_id = PluginId::new(
        plugin_name.to_string(),
        REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(),
    )
    .expect("plugin id");
    let root = PluginStore::new(codex_home.to_path_buf()).plugin_root(&plugin_id, "1.2.3");
    fs::create_dir_all(root.as_path()).expect("create cached remote plugin root");
    root
}
fn installed_remote_plugin_root(codex_home: &Path, plugin_name: &str) -> AbsolutePathBuf {
    let root = cached_remote_plugin_root(codex_home, plugin_name);
    let plugin_id = PluginId::new(
        plugin_name.to_string(),
        REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(),
    )
    .expect("plugin id");
    PluginStore::new(codex_home.to_path_buf())
        .write_remote_plugin_id(&plugin_id, "plugins~Plugin_sample")
        .expect("write remote plugin id");
    root
}
fn script_fixture() -> (TempDir, AbsolutePathBuf, AbsolutePathBuf) {
    let temp = TempDir::new().expect("temp dir");
    write_curated_plugin_sha_with(temp.path(), TEST_CURATED_PLUGIN_SHA);
    let root = synced_plugin_root(temp.path(), OPENAI_CURATED_MARKETPLACE_NAME, "sample");
    let script = root.join("scripts/run.py");
    fs::create_dir_all(script.as_path().parent().expect("script parent")).expect("create scripts");
    fs::write(script.as_path(), "#!/usr/bin/env python3\n").expect("write script");
    let script = script.canonicalize().expect("canonical script");
    (temp, root, script)
}

#[test]
fn resolves_primary_runtime_scripts_from_the_installed_plugin_cache() {
    let temp = TempDir::new().expect("temp dir");
    let marketplace_root = temp.path().join("openai-primary-runtime");
    let source_root = marketplace_root.join("plugins/presentations");
    fs::create_dir_all(source_root.join(".codex-plugin")).expect("create manifest directory");
    fs::write(
        source_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"presentations","version":"0.1.29"}"#,
    )
    .expect("write plugin manifest");
    let relative_script_path =
        "skills/presentations/container_tools/mark_artifact_operation_started.mjs";
    let source_script = source_root.join(relative_script_path);
    fs::create_dir_all(source_script.parent().expect("script parent")).expect("create scripts");
    fs::write(&source_script, "#!/usr/bin/env node\n").expect("write script");
    fs::create_dir_all(marketplace_root.join(".agents/plugins"))
        .expect("create marketplace directory");
    fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-primary-runtime",
  "plugins": [
    {
      "name": "presentations",
      "source": {
        "source": "local",
        "path": "./plugins/presentations"
      }
    }
  ]
}"#,
    )
    .expect("write marketplace manifest");
    let plugin_id = PluginId::parse("presentations@openai-primary-runtime").expect("plugin id");
    let plugin_root = PluginStore::new(temp.path().to_path_buf())
        .install(path(&source_root), plugin_id.clone())
        .expect("install plugin")
        .installed_path;
    let script = plugin_root.join(relative_script_path);
    let store = PluginStore::new(temp.path().to_path_buf());
    assert_eq!(
        TrustedPluginRoots::expected_plugin_root(
            &store,
            temp.path(),
            &plugin_id,
            Some(&marketplace_root),
        ),
        Some(plugin_root.clone())
    );
    let roots = TrustedPluginRoots {
        roots: vec![TrustedPluginRoot {
            plugin_id: plugin_id.clone(),
            metrics_operations_by_path: BTreeMap::new(),
            root: plugin_root.canonicalize().expect("canonical plugin root"),
        }],
    };

    assert_eq!(
        roots.resolve_attribution(
            &command(&[
                "node",
                script.to_string_lossy().as_ref(),
                "--operation-kind",
                "create",
            ]),
            &path(temp.path()),
        ),
        Some(PluginCommandAttribution {
            plugin_id,
            normalized_relative_path: relative_script_path.to_string(),
        })
    );
}
fn roots_for(codex_home: &Path, plugins: Vec<LoadedPlugin>) -> TrustedPluginRoots {
    TrustedPluginRoots::from_plugin_load_outcome(
        &PluginLoadOutcome::from_plugins(plugins),
        codex_home,
    )
}

#[tokio::test]
async fn resolves_relocated_script_through_executor_filesystem() {
    let (temp, root, script) = script_fixture();
    let roots = roots_for(
        temp.path(),
        vec![loaded_plugin(
            "sample@openai-curated",
            root.as_path(),
            ENABLED,
        )],
    );
    let executor = TempDir::new().expect("executor temp dir");
    let executor_root = executor
        .path()
        .join("plugins/cache/openai-curated/sample/remote-version");
    let executor_script = executor_root.join("scripts/run.py");
    fs::create_dir_all(executor_script.parent().expect("script parent"))
        .expect("create executor scripts");
    fs::copy(script.as_path(), &executor_script).expect("copy script to executor");
    let cwd = PathUri::from_host_native_path(&executor_root).expect("executor root URI");
    let environment =
        codex_exec_server::Environment::create_for_tests(/*exec_server_url*/ None)
            .expect("local executor environment");

    assert_eq!(
        roots
            .resolve_executor_attribution(
                &command(&["python", "scripts/run.py"]),
                &cwd,
                environment.get_filesystem().as_ref(),
            )
            .await,
        Some(PluginCommandAttribution {
            plugin_id: PluginId::parse("sample@openai-curated").expect("plugin id"),
            normalized_relative_path: "scripts/run.py".to_string(),
        })
    );

    fs::write(&executor_script, "print('modified')\n").expect("modify executor script");
    assert_eq!(
        roots
            .resolve_executor_attribution(
                &command(&["python", "scripts/run.py"]),
                &cwd,
                environment.get_filesystem().as_ref(),
            )
            .await,
        None
    );
}

#[test]
fn recognizes_windows_executor_plugin_cache_root() {
    let attribution = PluginCommandAttribution {
        plugin_id: PluginId::parse("presentations@openai-primary-runtime").expect("plugin id"),
        normalized_relative_path:
            "skills/presentations/container_tools/mark_artifact_operation_started.mjs".to_string(),
    };
    let script = PathUri::parse(
        "file:///C:/Users/user/.codex/plugins/cache/openai-primary-runtime/presentations/0.1.29/skills/presentations/container_tools/mark_artifact_operation_started.mjs",
    )
    .expect("Windows script URI");

    assert!(executor_plugin_root_matches(&script, &attribution));
}
fn assert_invalid_metrics_manifest(codex_home: &Path, root: &AbsolutePathBuf, manifest: &str) {
    fs::write(root.join("analytics.yaml"), manifest).expect("write analytics manifest");
    let roots = roots_for(
        codex_home,
        vec![loaded_plugin(
            "sample@openai-curated",
            root.as_path(),
            ENABLED,
        )],
    );
    roots
        .resolve_attribution(&command(&["scripts/run.py"]), root)
        .expect("Part 1 attribution remains enabled");
    assert_eq!(
        roots.resolve_metrics_operation(&command(&["scripts/run.py"]), root),
        None
    );
}
fn assert_untrusted(codex_home: &Path, config_name: &str, root: &Path) {
    assert!(
        roots_for(codex_home, vec![loaded_plugin(config_name, root, ENABLED)])
            .roots
            .is_empty()
    );
}
fn command(parts: &[&str]) -> Vec<String> {
    parts.iter().map(ToString::to_string).collect()
}

#[test]
fn trusted_roots_require_verified_curated_or_remote_cache() {
    let temp = TempDir::new().expect("temp dir");
    write_curated_plugin_sha_with(temp.path(), TEST_CURATED_PLUGIN_SHA);
    let root = synced_plugin_root(temp.path(), OPENAI_CURATED_MARKETPLACE_NAME, "sample");
    let api_root = synced_plugin_root(
        temp.path(),
        OPENAI_API_CURATED_MARKETPLACE_NAME,
        "api-sample",
    );
    let remote_root = installed_remote_plugin_root(temp.path(), "remote-sample");
    let unverified_remote_root = cached_remote_plugin_root(temp.path(), "unverified-remote");
    let _ = installed_remote_plugin_root(temp.path(), "overridden-remote");
    let overridden_remote_plugin_id =
        PluginId::parse("overridden-remote@openai-curated-remote").expect("plugin id");
    let remote_local_override = PluginStore::new(temp.path().to_path_buf())
        .plugin_root(&overridden_remote_plugin_id, DEFAULT_PLUGIN_VERSION);
    let local_root = temp
        .path()
        .join("plugins/cache/openai-curated/sample/local");
    let spoofed_root = temp.path().join("spoofed/openai-curated/sample");
    let spoofed_remote_root = temp
        .path()
        .join("spoofed/openai-curated-remote/remote-sample");
    fs::create_dir_all(&local_root).expect("create local root");
    fs::create_dir_all(&spoofed_root).expect("create spoofed root");
    fs::create_dir_all(&spoofed_remote_root).expect("create spoofed remote root");
    fs::create_dir_all(remote_local_override.as_path()).expect("create remote local override");
    let roots = roots_for(
        temp.path(),
        vec![
            loaded_plugin("sample@openai-curated", root.as_path(), ENABLED),
            loaded_plugin("api-sample@openai-api-curated", api_root.as_path(), ENABLED),
            loaded_plugin(
                "remote-sample@openai-curated-remote",
                remote_root.as_path(),
                ENABLED,
            ),
            loaded_plugin("sample@openai-curated", &local_root, ENABLED),
            loaded_plugin("sample@openai-curated", &spoofed_root, ENABLED),
            loaded_plugin("disabled@openai-curated", root.as_path(), DISABLED),
        ],
    );
    assert_eq!(
        roots.roots,
        vec![
            TrustedPluginRoot {
                plugin_id: PluginId::parse("sample@openai-curated").expect("plugin id"),
                metrics_operations_by_path: BTreeMap::new(),
                root: root.canonicalize().expect("canonical root"),
            },
            TrustedPluginRoot {
                plugin_id: PluginId::parse("api-sample@openai-api-curated").expect("plugin id"),
                metrics_operations_by_path: BTreeMap::new(),
                root: api_root.canonicalize().expect("canonical root"),
            },
            TrustedPluginRoot {
                plugin_id: PluginId::parse("remote-sample@openai-curated-remote")
                    .expect("plugin id"),
                metrics_operations_by_path: BTreeMap::new(),
                root: remote_root.canonicalize().expect("canonical root"),
            },
        ]
    );
    assert_untrusted(
        temp.path(),
        "unverified-remote@openai-curated-remote",
        unverified_remote_root.as_path(),
    );
    assert_untrusted(
        temp.path(),
        "remote-sample@openai-curated-remote",
        &spoofed_remote_root,
    );
    assert_untrusted(
        temp.path(),
        "overridden-remote@openai-curated-remote",
        remote_local_override.as_path(),
    );
    #[cfg(unix)]
    {
        let alias = temp.path().join("sample-alias");
        std::os::unix::fs::symlink(root.as_path(), &alias).expect("symlink root");
        assert_untrusted(temp.path(), "sample@openai-curated", &alias);
    }
    let _ = synced_plugin_root(temp.path(), OPENAI_CURATED_MARKETPLACE_NAME, "listed");
    let unlisted_root = PluginStore::new(temp.path().to_path_buf()).plugin_root(
        &PluginId::parse("missing@openai-curated").expect("plugin id"),
        &curated_plugin_cache_version(TEST_CURATED_PLUGIN_SHA),
    );
    fs::create_dir_all(unlisted_root.as_path()).expect("create unlisted root");
    assert_untrusted(
        temp.path(),
        "missing@openai-curated",
        unlisted_root.as_path(),
    );
    let no_sha = TempDir::new().expect("temp dir");
    let no_sha_root = synced_plugin_root(no_sha.path(), OPENAI_CURATED_MARKETPLACE_NAME, "sample");
    assert_untrusted(
        no_sha.path(),
        "sample@openai-curated",
        no_sha_root.as_path(),
    );
}

#[test]
fn resolves_manifest_operation_for_exact_attributed_script() {
    let (temp, root, _) = script_fixture();
    fs::write(
        root.join("analytics.yaml"),
        r#"version: 1
operations:
  security_scan:
    path: ./scripts/run.py
    measurements:
      repository_files: {}
      findings:
        dimensions:
          severity: [critical, high, medium, low]
"#,
    )
    .expect("write analytics manifest");
    let roots = roots_for(
        temp.path(),
        vec![loaded_plugin(
            "sample@openai-curated",
            root.as_path(),
            ENABLED,
        )],
    );
    assert_eq!(
        roots.resolve_metrics_operation(&command(&["scripts/run.py"]), &root),
        Some(ResolvedPluginMetricsOperation {
            plugin_id: PluginId::parse("sample@openai-curated").expect("plugin id"),
            operation: PluginMetricsOperation {
                operation_name: "security_scan".to_string(),
                measurements: BTreeMap::from([
                    (
                        "findings".to_string(),
                        PluginMeasurementDefinition {
                            enum_dimensions: BTreeMap::from([(
                                "severity".to_string(),
                                BTreeSet::from([
                                    "critical".to_string(),
                                    "high".to_string(),
                                    "low".to_string(),
                                    "medium".to_string(),
                                ]),
                            )]),
                        },
                    ),
                    (
                        "repository_files".to_string(),
                        PluginMeasurementDefinition {
                            enum_dimensions: BTreeMap::new(),
                        },
                    ),
                ]),
            },
        })
    );

    let undeclared_script = root.join("scripts/undeclared.py");
    fs::write(undeclared_script.as_path(), "print('ok')\n").expect("write undeclared script");
    roots
        .resolve_attribution(&command(&["scripts/undeclared.py"]), &root)
        .expect("trusted attribution");
    assert_eq!(
        roots.resolve_metrics_operation(&command(&["scripts/undeclared.py"]), &root),
        None
    );
}

#[test]
fn allows_measurement_names_reused_across_operations() {
    let (temp, root, _) = script_fixture();
    let other_script = root.join("scripts/other.py");
    fs::write(other_script.as_path(), "#!/usr/bin/env python3\n").expect("write script");
    fs::write(
        root.join("analytics.yaml"),
        r#"version: 1
operations:
  first:
    path: ./scripts/run.py
    measurements:
      count: {}
  second:
    path: ./scripts/other.py
    measurements:
      count: {}
"#,
    )
    .expect("write analytics manifest");
    let roots = roots_for(
        temp.path(),
        vec![loaded_plugin(
            "sample@openai-curated",
            root.as_path(),
            ENABLED,
        )],
    );

    for (script, operation_name) in [("scripts/run.py", "first"), ("scripts/other.py", "second")] {
        let resolved = roots
            .resolve_metrics_operation(&command(&[script]), &root)
            .expect("resolved metrics operation");
        assert_eq!(resolved.operation.operation_name, operation_name);
        assert!(resolved.operation.measurements.contains_key("count"));
    }
}

#[test]
fn invalid_manifest_disables_metrics_without_disabling_attribution() {
    let (temp, root, _) = script_fixture();
    let invalid_manifests = [
        r#"version: 2
operations: {scan: {path: scripts/run.py, measurements: {count: {}}}}
"#,
        r#"version: 1
unknown: true
operations: {scan: {path: scripts/run.py, measurements: {count: {}}}}
"#,
        r#"version: 1
operations:
  scan: {path: scripts/run.py, measurements: {count: {}}}
  scan: {path: scripts/run.py, measurements: {count: {}}}
"#,
        r#"version: 1
operations:
  scan:
    path: scripts/run.py
    measurements:
      count: {}
      count: {}
"#,
        r#"version: 1
operations:
  scan:
    path: ../outside.py
    measurements:
      count: {}
"#,
        r#"version: 1
operations: {scan: {path: scripts/run.py, measurements: {count: {dimensions: {status: ["needs review"]}}}}}
"#,
        r#"version: 1
operations: {BadName: {path: scripts/run.py, measurements: {count: {}}}}
"#,
        r#"version: 1
operations: {scan: {path: scripts/run.py, measurements: {count: {}}}, scan_again: {path: ./scripts/run.py, measurements: {count: {}}}}
"#,
    ];

    for manifest in invalid_manifests {
        assert_invalid_metrics_manifest(temp.path(), &root, manifest);
    }

    let oversized_manifest = format!(
        "version: 1\noperations: {{scan: {{path: scripts/run.py, measurements: {{count: {{}}}}}}}}\n#{}",
        "x".repeat(64 * 1024)
    );
    assert_invalid_metrics_manifest(temp.path(), &root, &oversized_manifest);

    #[cfg(unix)]
    {
        let outside = temp.path().join("outside.py");
        fs::write(&outside, "print('outside')\n").expect("write outside script");
        std::os::unix::fs::symlink(&outside, root.join("scripts/escape.py"))
            .expect("symlink script");
        assert_invalid_metrics_manifest(
            temp.path(),
            &root,
            r#"version: 1
operations:
  scan:
    path: scripts/escape.py
    measurements:
      count: {}
"#,
        );
    }
}

#[test]
fn resolves_local_attribution_for_safe_interpreters_and_wrappers() {
    let (temp, root, script) = script_fixture();
    let roots = roots_for(
        temp.path(),
        vec![loaded_plugin(
            "sample@openai-curated",
            root.as_path(),
            ENABLED,
        )],
    );
    let expected = Some(PluginCommandAttribution {
        plugin_id: PluginId::parse("sample@openai-curated").expect("plugin id"),
        normalized_relative_path: "scripts/run.py".to_string(),
    });
    let script = script.to_string_lossy().to_string();
    let unix_wrapper = format!("python -u {script}");
    for command in [
        command(&["scripts/run.py"]),
        command(&["/usr/bin/python", "-u", &script]),
        command(&["sh", "-e", &script]),
        command(&["bash", "-e", &script]),
        command(&["zsh", "-e", &script]),
        command(&["pwsh", "-File", &script]),
        command(&["powershell", "-File", &script]),
        command(&["bash", "-lc", &unix_wrapper]),
        command(&["pwsh.exe", "-NoProfile", "-Command", "scripts/run.py"]),
        command(&["cmd.exe", "/c", "scripts/run.py"]),
    ] {
        assert_eq!(roots.resolve_attribution(&command, &root), expected);
    }

    let wrapped_command = command(&[
        "bash",
        "-lc",
        &format!("node {script} --operation-kind create"),
    ]);
    assert_eq!(roots.resolve_attribution(&wrapped_command, &root), expected);
    assert_eq!(
        command_script_arguments(&wrapped_command),
        Some(command(&["--operation-kind", "create"]))
    );
}

#[test]
fn only_emits_safe_normalized_relative_script_paths() {
    assert_eq!(
        normalized_relative_script_path(Path::new("scripts/run.py")),
        Some("scripts/run.py".to_string())
    );
    assert_eq!(
        normalized_relative_script_path(Path::new(
            "/home/user/.codex/plugins/cache/openai-curated/sample/scripts/run.py"
        )),
        None
    );
}

#[test]
fn rejects_ambiguous_commands_overlaps_and_symlink_escapes() {
    let (temp, root, script) = script_fixture();
    let roots = roots_for(
        temp.path(),
        vec![loaded_plugin(
            "sample@openai-curated",
            root.as_path(),
            ENABLED,
        )],
    );
    let script = script.to_string_lossy().to_string();
    let complex = format!("python {script} && echo done");
    for command in [
        command(&["bash", "-lc", &complex]),
        command(&["node", "--require", "scripts/bootstrap.js", &script]),
        command(&["python", "-m", "scripts.run"]),
        command(&[
            "pwsh.exe",
            "-NoProfile",
            "-Command",
            "scripts/run.py; echo done",
        ]),
        command(&["python", "scripts/missing.py"]),
    ] {
        assert_eq!(roots.resolve_attribution(&command, &root), None);
    }
    let overlapping = TrustedPluginRoots {
        roots: vec![
            TrustedPluginRoot {
                plugin_id: PluginId::parse("sample@openai-curated").expect("plugin id"),
                metrics_operations_by_path: BTreeMap::new(),
                root: root.canonicalize().expect("canonical root"),
            },
            TrustedPluginRoot {
                plugin_id: PluginId::parse("nested@openai-curated").expect("plugin id"),
                metrics_operations_by_path: BTreeMap::new(),
                root: root.join("scripts").canonicalize().expect("nested root"),
            },
        ],
    };
    assert_eq!(
        overlapping.resolve_attribution(&command(&["scripts/run.py"]), &root),
        None
    );
    #[cfg(unix)]
    {
        let outside = temp.path().join("outside.py");
        fs::write(&outside, "print('outside')\n").expect("write outside script");
        std::os::unix::fs::symlink(&outside, root.join("scripts/escape.py")).expect("symlink");
        assert_eq!(
            roots.resolve_attribution(&command(&["python", "scripts/escape.py"]), &root),
            None
        );

        for unsafe_name in [r"scripts\run.py", "C:run.py"] {
            let unsafe_script = root.join(unsafe_name);
            fs::write(unsafe_script.as_path(), "print('unsafe')\n").expect("write unsafe script");
            assert_eq!(
                roots.resolve_attribution(
                    &command(&["python", &unsafe_script.to_string_lossy()]),
                    &root,
                ),
                None
            );
        }
    }
}
