use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_core_skills::HostSkillsSnapshot;
use codex_core_skills::loader::MAX_CONCURRENT_ROOT_SCANS;
use codex_core_skills::loader::SkillRoot;
use codex_core_skills::loader::load_skills_from_roots;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorCapabilityDiscoveryCache;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::SkillScope;
use codex_skills_extension::ExecutorSkillProvider;
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
    let alias_root = AbsolutePathBuf::from_absolute_path_checked(test_root.join("alias"))
        .expect("absolute path");
    let canonical_root = AbsolutePathBuf::from_absolute_path_checked(test_root.join("canonical"))
        .expect("absolute path");
    assert!(!alias_root.as_path().exists());
    assert!(!canonical_root.as_path().exists());

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: alias_root.clone(),
            scope: SkillScope::User,
            file_system: Arc::new(SyntheticFileSystem {
                alias_root: PathUri::from_abs_path(&alias_root),
                canonical_root: PathUri::from_abs_path(&canonical_root),
                has_plugin_manifest: false,
            }),
            plugin_id: None,
            plugin_namespace: None,
            plugin_root: None,
            discovery_mode: Default::default(),
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;
    assert_eq!(outcome.errors, Vec::new());
    assert_eq!(outcome.skills.len(), 1);

    let skill = outcome.skills[0].clone();
    assert_eq!(skill.name, "synthetic");
    assert_eq!(
        skill.path_to_skills_md,
        canonical_root.join("skill/SKILL.md")
    );
    let loaded = HostSkillsSnapshot::new(Arc::new(outcome));
    assert_eq!(
        loaded.read_skill_text(&skill).await.expect("skill body"),
        SKILL_CONTENTS
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
        .snapshot(&executor_roots)
        .await;
    let catalog = provider
        .list(SkillListQuery {
            turn_id: "turn-1".to_string(),
            executor_roots,
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
