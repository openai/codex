use std::io;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_core_skills::HostLoadedSkills;
use codex_core_skills::loader::SkillRoot;
use codex_core_skills::loader::load_skills_from_roots;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::SkillScope;
use codex_skills_extension::ExecutorSkillProvider;
use codex_skills_extension::provider::SkillListQuery;
use codex_skills_extension::provider::SkillProvider;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;

const SKILL_CONTENTS: &str =
    "---\nname: synthetic\ndescription: Synthetic executor skill.\n---\n\nEXECUTOR_ONLY_BODY\n";
static NEXT_TEST_ROOT_ID: AtomicUsize = AtomicUsize::new(0);

struct SyntheticFileSystem {
    alias_root: AbsolutePathBuf,
    canonical_root: AbsolutePathBuf,
}

impl SyntheticFileSystem {
    async fn canonicalize(&self, path: &PathUri) -> io::Result<PathUri> {
        let path = path.to_abs_path()?;
        if path == self.alias_root {
            return PathUri::from_abs_path(&self.canonical_root);
        }
        self.metadata(&path)?;
        PathUri::from_abs_path(&path)
    }

    async fn read_file(&self, path: &PathUri) -> io::Result<Vec<u8>> {
        if path.to_abs_path()? == self.canonical_root.join("skill/SKILL.md") {
            Ok(SKILL_CONTENTS.as_bytes().to_vec())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
        }
    }

    async fn read_directory(&self, path: &PathUri) -> io::Result<Vec<ReadDirectoryEntry>> {
        let path = path.to_abs_path()?;
        if path == self.canonical_root {
            Ok(vec![ReadDirectoryEntry {
                file_name: "skill".to_string(),
                is_directory: true,
                is_file: false,
            }])
        } else if path == self.canonical_root.join("skill") {
            Ok(vec![ReadDirectoryEntry {
                file_name: "SKILL.md".to_string(),
                is_directory: false,
                is_file: true,
            }])
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
        }
    }

    fn metadata(&self, path: &AbsolutePathBuf) -> io::Result<FileMetadata> {
        let skill_dir = self.canonical_root.join("skill");
        let skill_path = skill_dir.join("SKILL.md");
        let (is_directory, is_file) = if path == &self.canonical_root || path == &skill_dir {
            (true, false)
        } else if path == &skill_path {
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
        Box::pin(async move { self.metadata(&path.to_abs_path()?) })
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

    let outcome = load_skills_from_roots([SkillRoot {
        path: alias_root.clone(),
        scope: SkillScope::User,
        file_system: Arc::new(SyntheticFileSystem {
            alias_root,
            canonical_root: canonical_root.clone(),
        }),
        plugin_id: None,
        plugin_root: None,
    }])
    .await;
    assert_eq!(outcome.errors, Vec::new());
    assert_eq!(outcome.skills.len(), 1);

    let skill = outcome.skills[0].clone();
    assert_eq!(skill.name, "synthetic");
    assert_eq!(
        skill.path_to_skills_md,
        canonical_root.join("skill/SKILL.md")
    );
    let loaded = HostLoadedSkills::new(Arc::new(outcome));
    assert_eq!(
        loaded.read_skill_text(&skill).await.expect("skill body"),
        SKILL_CONTENTS
    );
}

#[tokio::test]
async fn selected_root_id_distinguishes_identical_executor_paths() {
    let test_root = create_local_skill_root("root-identity").expect("create local skill root");
    let root_path = test_root.to_string_lossy().into_owned();
    let canonical_root = AbsolutePathBuf::from_absolute_path_checked(&test_root)
        .expect("absolute skill root")
        .canonicalize()
        .expect("canonicalize skill root")
        .to_string_lossy()
        .replace('\\', "/");
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
                        path: root_path.clone(),
                    },
                })
                .collect(),
            host: None,
            include_host_skills: false,
            include_bundled_skills: true,
            include_orchestrator_skills: false,
            mcp_resources: None,
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
                    canonical_root.trim_start_matches('/')
                ),
            ),
            (
                "root-b".to_string(),
                format!(
                    "skill://root-b/{}/skill/SKILL.md",
                    canonical_root.trim_start_matches('/')
                ),
            ),
        ]
    );

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
