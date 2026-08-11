use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirementsToml;
use codex_exec_server::CapabilityRootDiscovery;
use codex_exec_server::CapabilityTextFile;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::DiscoveredSkillFiles;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorCapabilityDiscoveryCache;
use codex_exec_server::ExecutorCapabilityDiscoverySnapshot;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::Product;
use codex_skills_extension::ExecutorSkillProvider;
use codex_skills_extension::HostSkillsLoadInput;
use codex_skills_extension::HostSkillsService;
use codex_skills_extension::catalog::SkillAuthority;
use codex_skills_extension::catalog::SkillCatalog;
use codex_skills_extension::catalog::SkillCatalogEntry;
use codex_skills_extension::catalog::SkillPackageId;
use codex_skills_extension::catalog::SkillResourceId;
use codex_skills_extension::catalog::SkillSourceKind;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProvider;
use codex_skills_extension::provider::SkillReadRequest;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

const SKILL_CONTENTS: &str =
    "---\nname: synthetic\ndescription: Synthetic executor skill.\n---\n\nEXECUTOR_ONLY_BODY\n";
const PLUGIN_MANIFEST: &str = r#"{"name":"synthetic-plugin"}"#;
static NEXT_TEST_ROOT_ID: AtomicUsize = AtomicUsize::new(0);

struct SyntheticFileSystem {
    alias_root: PathUri,
    canonical_root: PathUri,
    has_plugin_manifest: bool,
}

impl SyntheticFileSystem {
    fn path(&self, relative_path: &str) -> io::Result<PathUri> {
        self.canonical_root
            .join(relative_path)
            .map_err(io::Error::other)
    }

    async fn canonicalize(&self, path: &PathUri) -> io::Result<PathUri> {
        if path == &self.alias_root {
            return Ok(self.canonical_root.clone());
        }
        self.metadata(path)?;
        Ok(path.clone())
    }

    async fn read_file(&self, path: &PathUri) -> io::Result<Vec<u8>> {
        if path == &self.path("skill/SKILL.md")? {
            Ok(SKILL_CONTENTS.as_bytes().to_vec())
        } else if self.has_plugin_manifest && path == &self.path(".claude-plugin/plugin.json")? {
            Ok(PLUGIN_MANIFEST.as_bytes().to_vec())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
        }
    }

    async fn read_directory(&self, path: &PathUri) -> io::Result<Vec<ReadDirectoryEntry>> {
        if path == &self.canonical_root {
            Ok(vec![ReadDirectoryEntry {
                file_name: "skill".to_string(),
                is_directory: true,
                is_file: false,
            }])
        } else if path == &self.path("skill")? {
            Ok(vec![ReadDirectoryEntry {
                file_name: "SKILL.md".to_string(),
                is_directory: false,
                is_file: true,
            }])
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
        }
    }

    fn metadata(&self, path: &PathUri) -> io::Result<FileMetadata> {
        let skill_dir = self.path("skill")?;
        let skill_path = self.path("skill/SKILL.md")?;
        let manifest_path = self.path(".claude-plugin/plugin.json")?;
        let (is_directory, is_file) = if path == &self.canonical_root || path == &skill_dir {
            (true, false)
        } else if path == &skill_path || self.has_plugin_manifest && path == &manifest_path {
            (false, true)
        } else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "not found"));
        };
        Ok(FileMetadata {
            is_directory,
            is_file,
            is_symlink: false,
            size: 0,
            created_at_ms: 0,
            modified_at_ms: 0,
        })
    }
}

impl ExecutorFileSystem for SyntheticFileSystem {
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri> {
        Box::pin(SyntheticFileSystem::canonicalize(self, path))
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        Box::pin(SyntheticFileSystem::read_file(self, path))
    }

    fn read_file_stream<'a>(
        &'a self,
        _path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        Box::pin(async {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "synthetic filesystem does not support streaming reads",
            ))
        })
    }

    fn write_file<'a>(
        &'a self,
        _path: &'a PathUri,
        _contents: Vec<u8>,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { Err(io::Error::new(io::ErrorKind::Unsupported, "read only")) })
    }

    fn create_directory<'a>(
        &'a self,
        _path: &'a PathUri,
        _options: CreateDirectoryOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { Err(io::Error::new(io::ErrorKind::Unsupported, "read only")) })
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(async move { self.metadata(path) })
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        Box::pin(SyntheticFileSystem::read_directory(self, path))
    }

    fn remove<'a>(
        &'a self,
        _path: &'a PathUri,
        _options: RemoveOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { Err(io::Error::new(io::ErrorKind::Unsupported, "read only")) })
    }

    fn copy<'a>(
        &'a self,
        _source_path: &'a PathUri,
        _destination_path: &'a PathUri,
        _options: CopyOptions,
        _sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(async move { Err(io::Error::new(io::ErrorKind::Unsupported, "read only")) })
    }
}

#[tokio::test]
async fn skill_loading_and_reads_use_the_supplied_executor_file_system() {
    let test_root =
        std::env::temp_dir().join(format!("codex-executor-skill-fs-{}", std::process::id()));
    let project_folder = AbsolutePathBuf::from_absolute_path_checked(test_root.join(".codex"))
        .expect("absolute project folder");
    let alias_root = project_folder.join("skills");
    let canonical_root = AbsolutePathBuf::from_absolute_path_checked(test_root.join("canonical"))
        .expect("absolute path");
    assert!(!alias_root.as_path().exists());
    assert!(!canonical_root.as_path().exists());

    let config_layer_stack = ConfigLayerStack::new(
        vec![ConfigLayerEntry::new(
            ConfigLayerSource::Project {
                dot_codex_folder: project_folder,
            },
            toml::from_str("[skills.bundled]\nenabled = false\n")
                .expect("valid bundled skills config"),
        )],
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid project config stack");
    let cwd = AbsolutePathBuf::from_absolute_path_checked(test_root).expect("absolute test root");
    let service = HostSkillsService::new_with_restriction_product(
        cwd.clone(),
        /*bundled_skills_enabled*/ false,
        /*restriction_product*/ None,
    );
    let snapshot = service
        .snapshot_for_config(
            &HostSkillsLoadInput::new(cwd, Vec::new(), config_layer_stack),
            Some(Arc::new(SyntheticFileSystem {
                alias_root: PathUri::from_abs_path(&alias_root),
                canonical_root: PathUri::from_abs_path(&canonical_root),
                has_plugin_manifest: false,
            })),
        )
        .await;
    let outcome = snapshot.outcome();
    assert_eq!(outcome.errors, Vec::new());
    assert_eq!(outcome.skills.len(), 1);

    let skill = outcome.skills[0].clone();
    assert_eq!(skill.name, "synthetic");
    assert_eq!(
        skill.path_to_skills_md,
        canonical_root.join("skill/SKILL.md")
    );
    assert_eq!(
        snapshot.read_skill_text(&skill).await.expect("skill body"),
        SKILL_CONTENTS
    );
}

#[tokio::test]
async fn windows_executor_skill_read_rejects_disabled_sandbox_on_any_orchestrator() {
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        Arc::new(EnvironmentManager::default_for_tests()),
        /*restriction_product*/ None,
    );
    let sandbox = FileSystemSandboxContext::from_permission_profile(
        PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::restricted(Vec::new()),
            NetworkSandboxPolicy::Restricted,
        ),
    );
    let resource = SkillResourceId::environment(
        "skill://windows-root/C:/skill/SKILL.md",
        "local",
        PathUri::parse("file:///C:/skill/SKILL.md").expect("Windows resource URI"),
    );
    let error = provider
        .read(SkillReadRequest {
            authority: SkillAuthority::new(SkillSourceKind::Executor, "windows-root"),
            package: SkillPackageId("skill://windows-root/C:/skill".into()),
            resource,
            resolved_executor_roots: Vec::new(),
            sandbox: Some(sandbox),
            host_snapshot: None,
            mcp_resources: None,
        })
        .await
        .expect_err("disabled Windows sandbox must fail closed");

    assert_eq!(
        error.message,
        "executor skill resource requires an unavailable filesystem sandbox"
    );
}

#[tokio::test]
async fn selected_root_id_distinguishes_identical_executor_paths() {
    let root_label = if cfg!(unix) {
        r"root\identity"
    } else {
        "root-identity"
    };
    let test_root = create_local_skill_root(root_label).expect("create local skill root");
    let selected_root = test_root.to_string_lossy().into_owned();
    let selected_root = if cfg!(windows) {
        selected_root.replace('\\', "/")
    } else {
        selected_root
    };
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        Arc::new(EnvironmentManager::default_for_tests()),
        /*restriction_product*/ None,
    );
    let catalog = provider
        .list(SkillListQuery {
            turn_id: "turn-1".to_string(),
            executor_roots: ["root-a", "root-b"]
                .into_iter()
                .map(|id| SelectedCapabilityRoot {
                    id: id.to_string(),
                    location: CapabilityRootLocation::Environment {
                        environment_id: "local".to_string(),
                        path: PathUri::from_host_native_path(&test_root).expect("skill root URI"),
                    },
                })
                .collect(),
            resolved_executor_roots: Vec::new(),
            host_snapshot: None,
            include_host_skills: false,
            include_bundled_skills: true,
            include_orchestrator_skills: false,
            mcp_resources: None,
            executor_capability_discovery: None,
        })
        .await
        .expect("list executor skills");

    assert_eq!(
        catalog
            .entries
            .iter()
            .map(|entry| (
                entry.authority.id.clone(),
                entry.display_path.clone().expect("display path"),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "root-a".to_string(),
                format!(
                    "skill://root-a/{}/skill/SKILL.md",
                    selected_root.trim_start_matches('/')
                ),
            ),
            (
                "root-b".to_string(),
                format!(
                    "skill://root-b/{}/skill/SKILL.md",
                    selected_root.trim_start_matches('/')
                ),
            ),
        ]
    );

    std::fs::remove_dir_all(test_root).expect("remove skill directory");
}

#[tokio::test]
async fn executor_discovery_preserves_posix_and_windows_locator_alias_roots() {
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        Arc::new(EnvironmentManager::default_for_tests()),
        /*restriction_product*/ None,
    );

    for (root_uri, alias_root) in [
        (
            "file:///workspace/skills",
            "skill://cross-platform-root/workspace/skills",
        ),
        (
            "file:///C:/workspace/skills",
            "skill://cross-platform-root/C:/workspace/skills",
        ),
    ] {
        let root = PathUri::parse(root_uri).expect("executor root URI");
        let skill_path = root.join("skill/SKILL.md").expect("executor skill URI");
        let selected_root = SelectedCapabilityRoot {
            id: "cross-platform-root".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "remote".to_string(),
                path: root.clone(),
            },
        };
        let discovery = ExecutorCapabilityDiscoverySnapshot::new(
            std::slice::from_ref(&selected_root),
            vec![Ok(Arc::new(CapabilityRootDiscovery {
                id: selected_root.id.clone(),
                path: root,
                plugin: None,
                skills: vec![DiscoveredSkillFiles {
                    instructions: CapabilityTextFile {
                        path: skill_path.clone(),
                        contents: SKILL_CONTENTS.to_string(),
                    },
                    metadata: None,
                }],
                namespace_manifests: Vec::new(),
                warnings: Vec::new(),
                error: None,
            }))],
            HashMap::new(),
        );
        let catalog = provider
            .list(SkillListQuery {
                turn_id: "turn-1".to_string(),
                executor_roots: vec![selected_root],
                resolved_executor_roots: Vec::new(),
                host_snapshot: None,
                include_host_skills: false,
                include_bundled_skills: true,
                include_orchestrator_skills: false,
                mcp_resources: None,
                executor_capability_discovery: Some(discovery),
            })
            .await
            .expect("list executor skills");
        let resource = format!("{alias_root}/skill/SKILL.md");

        assert_eq!(
            catalog,
            SkillCatalog {
                entries: vec![
                    SkillCatalogEntry::new(
                        SkillPackageId(format!("{alias_root}/skill")),
                        SkillAuthority::new(SkillSourceKind::Executor, "cross-platform-root"),
                        "synthetic",
                        "Synthetic executor skill.",
                        SkillResourceId::environment_with_contents(
                            resource.clone(),
                            "remote",
                            skill_path,
                            SKILL_CONTENTS.to_string(),
                        ),
                    )
                    .with_display_path(resource)
                    .with_alias_root(alias_root)
                ],
                warnings: Vec::new(),
            }
        );
    }
}

#[tokio::test]
async fn executor_discovery_routes_produce_equivalent_catalog_metadata() {
    let id = NEXT_TEST_ROOT_ID.fetch_add(1, Ordering::Relaxed);
    let test_root = std::env::temp_dir().join(format!(
        "codex-executor-skill-parity-{}-{id}",
        std::process::id()
    ));
    let plugin_manifest = test_root.join(".codex-plugin/plugin.json");
    let deploy_skill = test_root.join("skills/deploy/SKILL.md");
    let deploy_metadata = test_root.join("skills/deploy/agents/openai.yaml");
    let excluded_skill = test_root.join("skills/excluded/SKILL.md");
    let excluded_metadata = test_root.join("skills/excluded/agents/openai.yaml");
    let repaired_skill = test_root.join("skills/repaired/SKILL.md");
    let invalid_skill = test_root.join("skills/invalid/SKILL.md");
    let invalid_metadata_skill = test_root.join("skills/invalid-metadata/SKILL.md");
    let invalid_metadata = test_root.join("skills/invalid-metadata/agents/openai.yaml");
    for path in [
        &plugin_manifest,
        &deploy_skill,
        &deploy_metadata,
        &excluded_skill,
        &excluded_metadata,
        &repaired_skill,
        &invalid_skill,
        &invalid_metadata_skill,
        &invalid_metadata,
    ] {
        std::fs::create_dir_all(path.parent().expect("test file parent"))
            .expect("create test directory");
    }
    std::fs::write(&plugin_manifest, r#"{"name":"catalog"}"#).expect("write plugin manifest");
    std::fs::write(
        &deploy_skill,
        "---\nname: deploy\ndescription: Deploy the service.\nmetadata:\n  short-description: Deploy safely.\n---\n\nDeploy.\n",
    )
    .expect("write deploy skill");
    std::fs::write(
        &deploy_metadata,
        "dependencies:\n  tools:\n    - type: mcp\n      value: deployer\n      description: Deployment server.\npolicy:\n  allow_implicit_invocation: false\n  products:\n    - codex\n",
    )
    .expect("write deploy metadata");
    std::fs::write(
        &excluded_skill,
        "---\nname: excluded\ndescription: Excluded skill.\n---\n",
    )
    .expect("write excluded skill");
    std::fs::write(&excluded_metadata, "policy:\n  products:\n    - chatgpt\n")
        .expect("write excluded metadata");
    std::fs::write(
        &repaired_skill,
        "---\ndescription: Build for AWS: ECS\n---\n",
    )
    .expect("write repaired skill");
    std::fs::write(&invalid_skill, "---\nname: invalid\n---\n").expect("write invalid skill");
    std::fs::write(
        &invalid_metadata_skill,
        "---\nname: invalid-metadata\ndescription: Invalid optional metadata.\n---\n",
    )
    .expect("write invalid metadata skill");
    std::fs::write(
        &invalid_metadata,
        "interface: []\ndependencies:\n  tools:\n    - type: mcp\n      value: must-be-ignored\n",
    )
    .expect("write invalid metadata");

    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        Arc::clone(&manager),
        Some(Product::Codex),
    );
    let executor_roots = vec![SelectedCapabilityRoot {
        id: "parity-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "local".to_string(),
            path: PathUri::from_host_native_path(&test_root).expect("skill root URI"),
        },
    }];
    let query = |executor_capability_discovery| SkillListQuery {
        turn_id: "turn-1".to_string(),
        executor_roots: executor_roots.clone(),
        resolved_executor_roots: Vec::new(),
        host_snapshot: None,
        include_host_skills: false,
        include_bundled_skills: true,
        include_orchestrator_skills: false,
        mcp_resources: None,
        executor_capability_discovery,
    };

    let direct = provider
        .list(query(None))
        .await
        .expect("list directly discovered executor skills");
    let discovery = ExecutorCapabilityDiscoveryCache::new(manager)
        .snapshot(&executor_roots, &Default::default())
        .await;
    let bundled = provider
        .list(query(Some(discovery)))
        .await
        .expect("list bundled executor skills");

    assert_eq!(bundled.warnings, direct.warnings);
    let comparable_entries = |catalog: &codex_skills_extension::catalog::SkillCatalog| {
        catalog
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.id.clone(),
                    entry.authority.clone(),
                    entry.name.clone(),
                    entry.description.clone(),
                    entry.short_description.clone(),
                    entry.display_path.clone(),
                    entry.dependencies.clone(),
                    entry.enabled,
                    entry.prompt_visible,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(comparable_entries(&bundled), comparable_entries(&direct));
    assert_eq!(direct.warnings.len(), 1);
    assert!(direct.warnings[0].contains("missing field `description`"));
    let entry = direct
        .entries
        .iter()
        .find(|entry| entry.name == "catalog:deploy")
        .expect("deploy skill");
    assert_eq!(
        entry
            .dependencies
            .as_ref()
            .expect("skill dependencies")
            .tools[0]
            .value,
        "deployer"
    );
    assert!(!entry.prompt_visible);
    let repaired = direct
        .entries
        .iter()
        .find(|entry| entry.name == "catalog:repaired")
        .expect("repaired skill");
    assert_eq!(repaired.description, "Build for AWS: ECS");
    let invalid_metadata = direct
        .entries
        .iter()
        .find(|entry| entry.name == "catalog:invalid-metadata")
        .expect("invalid metadata skill");
    assert_eq!(invalid_metadata.dependencies, None);

    std::fs::remove_dir_all(test_root).expect("remove skill directory");
}

#[tokio::test]
async fn pre_discovered_executor_catalog_snapshot() {
    let test_root = create_local_skill_root("snapshot").expect("create local skill root");
    let manifest_dir = test_root.join(".codex-plugin");
    let metadata_dir = test_root.join("skill/agents");
    std::fs::create_dir_all(&manifest_dir).expect("create plugin manifest directory");
    std::fs::create_dir_all(&metadata_dir).expect("create skill metadata directory");
    std::fs::write(
        manifest_dir.join("plugin.json"),
        r#"{"name":"snapshot-plugin"}"#,
    )
    .expect("write plugin manifest");
    std::fs::write(
        metadata_dir.join("openai.yaml"),
        "dependencies:\n  tools:\n    - type: mcp\n      value: deployer\n      description: Deployment server.\npolicy:\n  allow_implicit_invocation: false\n",
    )
    .expect("write skill metadata");

    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        Arc::clone(&manager),
        /*restriction_product*/ None,
    );
    let executor_roots = vec![SelectedCapabilityRoot {
        id: "snapshot-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "local".to_string(),
            path: PathUri::from_host_native_path(&test_root).expect("skill root URI"),
        },
    }];
    let executor_capability_discovery = ExecutorCapabilityDiscoveryCache::new(manager)
        .snapshot(&executor_roots, &Default::default())
        .await;
    let catalog = provider
        .list(SkillListQuery {
            turn_id: "turn-1".to_string(),
            executor_roots,
            resolved_executor_roots: Vec::new(),
            host_snapshot: None,
            include_host_skills: false,
            include_bundled_skills: true,
            include_orchestrator_skills: false,
            mcp_resources: None,
            executor_capability_discovery: Some(executor_capability_discovery),
        })
        .await
        .expect("list pre-discovered executor skills");

    let mut snapshot = serde_json::json!({
        "warnings": &catalog.warnings,
        "entries": catalog
            .entries
            .iter()
            .map(|entry| serde_json::json!({
                "authority": {
                    "kind": entry.authority.kind.to_string(),
                    "id": &entry.authority.id,
                },
                "name": &entry.name,
                "description": &entry.description,
                "short_description": entry.short_description.as_deref(),
                "dependencies": entry.dependencies.as_ref().map(|dependencies| {
                    dependencies.tools.iter().map(|tool| serde_json::json!({
                        "type": &tool.r#type,
                        "value": &tool.value,
                        "description": tool.description.as_deref(),
                        "transport": tool.transport.as_deref(),
                        "command": tool.command.as_deref(),
                        "url": tool.url.as_deref(),
                    })).collect::<Vec<_>>()
                }),
                "enabled": entry.enabled,
                "prompt_visible": entry.prompt_visible,
            }))
            .collect::<Vec<_>>(),
    });
    snapshot.sort_all_objects();
    let snapshot_file = codex_utils_cargo_bin::find_resource!(
        "tests/snapshots/executor_file_system_authority__pre_discovered_executor_catalog.snap"
    )
    .expect("resolve catalog snapshot");
    let snapshot_dir = snapshot_file
        .parent()
        .unwrap_or_else(|| panic!("snapshot file has no parent: {}", snapshot_file.display()));
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(snapshot_dir);
    settings.bind(|| {
        insta::assert_snapshot!(
            "pre_discovered_executor_catalog",
            serde_json::to_string_pretty(&snapshot).expect("serialize catalog snapshot")
        );
    });

    std::fs::remove_dir_all(test_root).expect("remove skill directory");
}

#[tokio::test]
async fn direct_executor_discovery_preserves_hidden_nested_and_probed_metadata() {
    let id = NEXT_TEST_ROOT_ID.fetch_add(1, Ordering::Relaxed);
    let test_root = std::env::temp_dir().join(format!(
        "codex-executor-skill-discovery-{}-{id}",
        std::process::id()
    ));
    let outer_manifest = test_root.join(".codex-plugin/plugin.json");
    let hidden_skill = test_root.join(".hidden/deploy/SKILL.md");
    let hidden_metadata = test_root.join(".hidden/deploy/agents/openai.yaml");
    let inner_manifest = test_root.join("nested/.codex-plugin/plugin.json");
    let inner_skill = test_root.join("nested/skills/audit/SKILL.md");
    for (path, contents) in [
        (&outer_manifest, r#"{"name":"outer"}"#),
        (
            &hidden_skill,
            "---\nname: hidden\ndescription: Hidden skill.\n---\n",
        ),
        (&inner_manifest, r#"{"name":"inner"}"#),
        (
            &inner_skill,
            "---\nname: audit\ndescription: Audit skill.\n---\n",
        ),
    ] {
        std::fs::create_dir_all(path.parent().expect("test file parent"))
            .expect("create test directory");
        std::fs::write(path, contents).expect("write test file");
    }
    std::fs::create_dir_all(hidden_metadata.parent().expect("metadata parent"))
        .expect("create metadata directory");
    let metadata_contents = "policy:\n  allow_implicit_invocation: false\n";
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        let metadata_target = test_root.join("linked-openai.yaml");
        std::fs::write(&metadata_target, metadata_contents).expect("write metadata target");
        symlink(metadata_target, &hidden_metadata).expect("link metadata");
    }
    #[cfg(not(unix))]
    std::fs::write(&hidden_metadata, metadata_contents).expect("write metadata");

    let root_uri = PathUri::from_host_native_path(&test_root).expect("skill root URI");
    let selected_root = SelectedCapabilityRoot {
        id: "discovery-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "local".to_string(),
            path: root_uri.clone(),
        },
    };
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        manager, /*restriction_product*/ None,
    );
    let direct = provider
        .list(SkillListQuery {
            turn_id: "turn-1".to_string(),
            executor_roots: vec![selected_root],
            resolved_executor_roots: Vec::new(),
            host_snapshot: None,
            include_host_skills: false,
            include_bundled_skills: true,
            include_orchestrator_skills: false,
            mcp_resources: None,
            executor_capability_discovery: None,
        })
        .await
        .expect("list directly discovered executor skills");

    assert_eq!(direct.warnings, Vec::<String>::new());
    let catalog_metadata = direct
        .entries
        .iter()
        .map(|entry| {
            (
                entry.name.clone(),
                entry.description.clone(),
                entry.prompt_visible,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        catalog_metadata,
        vec![
            ("inner:audit".to_string(), "Audit skill.".to_string(), true),
            (
                "outer:hidden".to_string(),
                "Hidden skill.".to_string(),
                false,
            ),
        ]
    );

    std::fs::remove_dir_all(test_root).expect("remove skill directory");
}

#[tokio::test]
async fn high_level_discovery_reuses_materialized_skill_contents_for_reads() {
    let test_root = create_local_skill_root("materialized").expect("create local skill root");
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let provider = ExecutorSkillProvider::new_with_restriction_product(
        Arc::clone(&manager),
        /*restriction_product*/ None,
    );
    let executor_roots = vec![SelectedCapabilityRoot {
        id: "materialized-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "local".to_string(),
            path: PathUri::from_host_native_path(&test_root).expect("skill root URI"),
        },
    }];
    let executor_capability_discovery = ExecutorCapabilityDiscoveryCache::new(manager)
        .snapshot(&executor_roots, &Default::default())
        .await;
    let catalog = provider
        .list(SkillListQuery {
            turn_id: "turn-1".to_string(),
            executor_roots,
            resolved_executor_roots: Vec::new(),
            host_snapshot: None,
            include_host_skills: false,
            include_bundled_skills: true,
            include_orchestrator_skills: false,
            mcp_resources: None,
            executor_capability_discovery: Some(executor_capability_discovery),
        })
        .await
        .expect("list executor skills");
    let [entry] = catalog.entries.as_slice() else {
        panic!("expected exactly one skill");
    };
    let request = SkillReadRequest {
        authority: entry.authority.clone(),
        package: entry.id.clone(),
        resource: entry.main_prompt.clone(),
        resolved_executor_roots: Vec::new(),
        sandbox: None,
        host_snapshot: None,
        mcp_resources: None,
    };

    std::fs::remove_dir_all(&test_root).expect("remove skill directory after discovery");
    let read = provider
        .read(request)
        .await
        .expect("read materialized executor skill");

    assert_eq!(read.contents, SKILL_CONTENTS);
}

#[tokio::test]
async fn high_level_discovery_cache_separates_filesystem_permission_contexts() {
    let test_root = create_local_skill_root("permission-context").expect("create local skill root");
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let cache = ExecutorCapabilityDiscoveryCache::new(manager);
    let executor_roots = vec![SelectedCapabilityRoot {
        id: "permission-root".to_string(),
        location: CapabilityRootLocation::Environment {
            environment_id: "local".to_string(),
            path: PathUri::from_host_native_path(&test_root).expect("skill root URI"),
        },
    }];
    cache.snapshot(&executor_roots, &HashMap::new()).await;

    let updated_contents =
        "---\nname: synthetic\ndescription: Updated executor skill.\n---\n\nUPDATED_BODY\n";
    std::fs::write(test_root.join("skill/SKILL.md"), updated_contents)
        .expect("update executor skill");
    let sandbox_contexts = HashMap::from([(
        "local".to_string(),
        FileSystemSandboxContext::from_permission_profile(PermissionProfile::Disabled),
    )]);
    let second_snapshot = cache.snapshot(&executor_roots, &sandbox_contexts).await;
    let second_discovery = second_snapshot.roots()[0]
        .result
        .as_ref()
        .expect("second discovery");

    assert_eq!(
        second_discovery.skills[0].instructions.contents,
        updated_contents
    );
    assert_eq!(second_snapshot.sandbox_contexts(), &sandbox_contexts);

    let restored_contents =
        "---\nname: synthetic\ndescription: Restored executor skill.\n---\n\nRESTORED_BODY\n";
    std::fs::write(test_root.join("skill/SKILL.md"), restored_contents)
        .expect("restore executor skill");
    let restored_snapshot = cache.snapshot(&executor_roots, &HashMap::new()).await;
    let restored_discovery = restored_snapshot.roots()[0]
        .result
        .as_ref()
        .expect("restored discovery");

    assert_eq!(
        restored_discovery.skills[0].instructions.contents,
        restored_contents
    );

    std::fs::remove_dir_all(test_root).expect("remove skill directory");
}

#[tokio::test]
async fn high_level_discovery_batches_more_than_128_roots() {
    let test_root = create_local_skill_root("many-roots").expect("create local skill root");
    let manager = Arc::new(EnvironmentManager::default_for_tests());
    let cache = ExecutorCapabilityDiscoveryCache::new(manager);
    let executor_roots = (0..129)
        .map(|index| SelectedCapabilityRoot {
            id: format!("root-{index}"),
            location: CapabilityRootLocation::Environment {
                environment_id: "local".to_string(),
                path: PathUri::from_host_native_path(&test_root).expect("skill root URI"),
            },
        })
        .collect::<Vec<_>>();

    let snapshot = cache.snapshot(&executor_roots, &HashMap::new()).await;

    assert_eq!(snapshot.roots().len(), executor_roots.len());
    assert!(snapshot.roots().iter().all(|root| root.result.is_ok()));

    std::fs::remove_dir_all(test_root).expect("remove skill directory");
}

fn create_local_skill_root(label: &str) -> io::Result<std::path::PathBuf> {
    let id = NEXT_TEST_ROOT_ID.fetch_add(1, Ordering::Relaxed);
    let test_root = std::env::temp_dir().join(format!(
        "codex-executor-skill-{label}-{}-{id}",
        std::process::id()
    ));
    let skill_dir = test_root.join("skill");
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(skill_dir.join("SKILL.md"), SKILL_CONTENTS)?;
    Ok(test_root)
}
