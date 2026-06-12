use super::*;
use crate::config::ConfigBuilder;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirements;
use codex_config::ConfigRequirementsToml;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LOCAL_FS;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_extension_api::UserInstructions;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use core_test_support::PathBufExt;
use core_test_support::TempDirExt;
use core_test_support::create_directory_symlink;
use pretty_assertions::assert_eq;
use std::fs;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;

#[derive(Clone, Copy)]
enum InjectedFailure {
    Metadata(io::ErrorKind),
    Read(io::ErrorKind),
}

struct FailingFileSystem {
    path: AbsolutePathBuf,
    failure: InjectedFailure,
}

impl FailingFileSystem {
    async fn canonicalize(
        &self,
        _path: &PathUri,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<PathUri> {
        unreachable!("canonicalize should not be called")
    }

    async fn read_file(
        &self,
        path: &PathUri,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Vec<u8>> {
        if path.to_abs_path()? == self.path
            && let InjectedFailure::Read(kind) = self.failure
        {
            return Err(io::Error::new(kind, "injected read failure"));
        }
        LOCAL_FS.read_file(path, sandbox).await
    }

    async fn write_file(
        &self,
        _path: &PathUri,
        _contents: Vec<u8>,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("write_file should not be called")
    }

    async fn create_directory(
        &self,
        _path: &PathUri,
        _create_directory_options: CreateDirectoryOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("create_directory should not be called")
    }

    async fn get_metadata(
        &self,
        path: &PathUri,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<FileMetadata> {
        if path.to_abs_path()? == self.path
            && let InjectedFailure::Metadata(kind) = self.failure
        {
            return Err(io::Error::new(kind, "injected metadata failure"));
        }
        LOCAL_FS.get_metadata(path, sandbox).await
    }

    async fn read_directory(
        &self,
        _path: &PathUri,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Vec<ReadDirectoryEntry>> {
        unreachable!("read_directory should not be called")
    }

    async fn remove(
        &self,
        _path: &PathUri,
        _remove_options: RemoveOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("remove should not be called")
    }

    async fn copy(
        &self,
        _source_path: &PathUri,
        _destination_path: &PathUri,
        _copy_options: CopyOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<()> {
        unreachable!("copy should not be called")
    }
}

impl ExecutorFileSystem for FailingFileSystem {
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri> {
        Box::pin(FailingFileSystem::canonicalize(self, path, sandbox))
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        Box::pin(FailingFileSystem::read_file(self, path, sandbox))
    }

    fn write_file<'a>(
        &'a self,
        path: &'a PathUri,
        contents: Vec<u8>,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(FailingFileSystem::write_file(self, path, contents, sandbox))
    }

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        options: CreateDirectoryOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(FailingFileSystem::create_directory(
            self, path, options, sandbox,
        ))
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(FailingFileSystem::get_metadata(self, path, sandbox))
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        Box::pin(FailingFileSystem::read_directory(self, path, sandbox))
    }

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        options: RemoveOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(FailingFileSystem::remove(self, path, options, sandbox))
    }

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        options: CopyOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        Box::pin(FailingFileSystem::copy(
            self,
            source_path,
            destination_path,
            options,
            sandbox,
        ))
    }
}

struct TestConfig {
    config: Config,
    user_instructions: Option<UserInstructions>,
}

impl Deref for TestConfig {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl DerefMut for TestConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

async fn get_user_instructions(config: &TestConfig) -> Option<String> {
    let mut warnings = Vec::new();
    load_agents_md(config, &mut warnings)
        .await
        .map(|loaded| loaded.text())
}

async fn load_agents_md(config: &TestConfig, warnings: &mut Vec<String>) -> Option<LoadedAgentsMd> {
    let mut core_config = config.config.clone();
    let existing_warning_count = core_config.startup_warnings.len();
    let loaded = load_project_instructions(
        &mut core_config,
        config.user_instructions.clone(),
        Some(LOCAL_FS.as_ref()),
    )
    .await;
    warnings.extend(
        core_config
            .startup_warnings
            .into_iter()
            .skip(existing_warning_count),
    );
    loaded
}

async fn agents_md_paths(config: &TestConfig) -> std::io::Result<Vec<AbsolutePathBuf>> {
    super::agents_md_paths(&config.config, LOCAL_FS.as_ref()).await
}

fn assert_invalid_utf8_warning(warnings: &[String], source: &str, path: &Path) {
    let path_display = path.display().to_string();
    assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
    let warning = &warnings[0];
    assert!(
        warning.contains(&format!("{source} AGENTS.md instructions"))
            && warning.contains(&path_display)
            && warning.contains("invalid UTF-8")
            && warning.contains("Invalid byte sequences were replaced."),
        "unexpected invalid UTF-8 warning: {warning:?}"
    );
}

/// Helper that returns a `Config` pointing at `root` and using `limit` as
/// the maximum number of bytes to embed from AGENTS.md. The caller can
/// optionally specify a custom `instructions` string – when `None` the
/// value is cleared to mimic a scenario where no system instructions have
/// been configured.
async fn make_config(root: &TempDir, limit: usize, instructions: Option<&str>) -> TestConfig {
    let codex_home = TempDir::new().unwrap();
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("defaults for test should always succeed");

    config.cwd = root.abs();
    config.project_doc_max_bytes = limit;

    let user_instructions = instructions.map(|text| UserInstructions {
        text: text.to_owned(),
        source: config.codex_home.join(DEFAULT_AGENTS_MD_FILENAME),
    });
    TestConfig {
        config,
        user_instructions,
    }
}

async fn make_config_with_fallback(
    root: &TempDir,
    limit: usize,
    instructions: Option<&str>,
    fallbacks: &[&str],
) -> TestConfig {
    let mut config = make_config(root, limit, instructions).await;
    config.project_doc_fallback_filenames = fallbacks
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    config
}

async fn make_config_with_project_root_markers(
    root: &TempDir,
    limit: usize,
    instructions: Option<&str>,
    markers: &[&str],
) -> TestConfig {
    let codex_home = TempDir::new().unwrap();
    let cli_overrides = vec![(
        "project_root_markers".to_string(),
        TomlValue::Array(
            markers
                .iter()
                .map(|marker| TomlValue::String((*marker).to_string()))
                .collect(),
        ),
    )];
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .cli_overrides(cli_overrides)
        .build()
        .await
        .expect("defaults for test should always succeed");

    config.cwd = root.abs();
    config.project_doc_max_bytes = limit;
    let user_instructions = instructions.map(|text| UserInstructions {
        text: text.to_owned(),
        source: config.codex_home.join(DEFAULT_AGENTS_MD_FILENAME),
    });
    TestConfig {
        config,
        user_instructions,
    }
}

/// AGENTS.md missing – should yield `None`.
#[tokio::test]
async fn no_doc_file_returns_none() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await)
            .await;
    assert!(
        res.is_none(),
        "Expected None when AGENTS.md is absent and no system instructions provided"
    );
    assert!(res.is_none(), "Expected None when AGENTS.md is absent");
}

#[test]
fn empty_loaded_instructions_are_empty() {
    let source =
        AbsolutePathBuf::from_absolute_path("/tmp/AGENTS.md").expect("absolute source path");

    assert_eq!(
        LoadedAgentsMd::new_user(String::new(), source.clone()),
        LoadedAgentsMd::default()
    );
    assert_eq!(
        LoadedAgentsMd::new_user(" \n\t".to_string(), source),
        LoadedAgentsMd::default()
    );
    assert_eq!(
        LoadedAgentsMd::from_text_for_testing(String::new()),
        LoadedAgentsMd::default()
    );
    assert_eq!(
        LoadedAgentsMd::from_text_for_testing(" \n\t"),
        LoadedAgentsMd::default()
    );
}

#[test]
fn loaded_instructions_with_only_empty_or_whitespace_entries_are_empty() {
    let empty = LoadedAgentsMd {
        user_instructions: None,
        entries: vec![InstructionEntry {
            contents: String::new(),
            provenance: InstructionProvenance::Internal,
        }],
    };
    let whitespace = LoadedAgentsMd {
        user_instructions: None,
        entries: vec![InstructionEntry {
            contents: " \n\t".to_string(),
            provenance: InstructionProvenance::Internal,
        }],
    };

    assert!(empty.is_empty());
    assert!(whitespace.is_empty());
}

/// Small file within the byte-limit is returned unmodified.
#[tokio::test]
async fn doc_smaller_than_limit_is_returned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "hello world").unwrap();

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await)
            .await
            .expect("doc expected");

    assert_eq!(
        res, "hello world",
        "The document should be returned verbatim when it is smaller than the limit and there are no existing instructions"
    );
}

#[tokio::test]
async fn project_doc_invalid_utf8_warns_and_uses_lossy_text() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("AGENTS.md");
    fs::write(&path, b"project\xFF doc").unwrap();

    let config = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    let mut warnings = Vec::new();
    let res = load_agents_md(&config, &mut warnings)
        .await
        .expect("doc expected")
        .text();

    assert_eq!(res, "project\u{FFFD} doc");
    assert_invalid_utf8_warning(&warnings, "Project", config.cwd.join("AGENTS.md").as_path());
}

/// Oversize file is truncated to `project_doc_max_bytes`.
#[tokio::test]
async fn doc_larger_than_limit_is_truncated() {
    const LIMIT: usize = 1024;
    let tmp = tempfile::tempdir().expect("tempdir");

    let huge = "A".repeat(LIMIT * 2); // 2 KiB
    fs::write(tmp.path().join("AGENTS.md"), &huge).unwrap();

    let res = get_user_instructions(&make_config(&tmp, LIMIT, /*instructions*/ None).await)
        .await
        .expect("doc expected");

    assert_eq!(res.len(), LIMIT, "doc should be truncated to LIMIT bytes");
    assert_eq!(res, huge[..LIMIT]);
}

#[tokio::test]
async fn total_byte_limit_truncates_later_project_docs() {
    let repo = tempfile::tempdir().expect("tempdir");
    fs::write(repo.path().join(".git"), "").unwrap();
    fs::write(repo.path().join("AGENTS.md"), "root").unwrap();
    let nested = repo.path().join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("AGENTS.md"), "abcdef").unwrap();

    let mut config = make_config(&repo, /*limit*/ 7, /*instructions*/ None).await;
    config.cwd = nested.abs();

    let mut warnings = Vec::new();
    let loaded = load_agents_md(&config, &mut warnings)
        .await
        .expect("project instructions");
    let expected = LoadedAgentsMd {
        user_instructions: None,
        entries: vec![
            InstructionEntry {
                contents: "root".to_string(),
                provenance: InstructionProvenance::Project(repo.path().join("AGENTS.md").abs()),
            },
            InstructionEntry {
                contents: "abc".to_string(),
                provenance: InstructionProvenance::Project(config.cwd.join("AGENTS.md")),
            },
        ],
    };

    assert_eq!(loaded, expected);
    assert_eq!(loaded.text(), "root\n\nabc");
    assert_eq!(warnings, Vec::<String>::new());
}

#[tokio::test]
async fn read_agents_md_propagates_metadata_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut config = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    let marker_path = config.cwd.join(".git");
    let fs = FailingFileSystem {
        path: marker_path,
        failure: InjectedFailure::Metadata(io::ErrorKind::PermissionDenied),
    };

    let err = read_agents_md(&mut config.config, &fs)
        .await
        .expect_err("metadata error");

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
}

#[tokio::test]
async fn read_agents_md_propagates_read_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "project doc").unwrap();
    let mut config = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    let fs = FailingFileSystem {
        path: config.cwd.join("AGENTS.md"),
        failure: InjectedFailure::Read(io::ErrorKind::PermissionDenied),
    };

    let err = read_agents_md(&mut config.config, &fs)
        .await
        .expect_err("read error");

    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
}

#[tokio::test]
async fn read_agents_md_ignores_files_removed_after_discovery() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "project doc").unwrap();
    let mut config = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    let fs = FailingFileSystem {
        path: config.cwd.join("AGENTS.md"),
        failure: InjectedFailure::Read(io::ErrorKind::NotFound),
    };

    let loaded = read_agents_md(&mut config.config, &fs)
        .await
        .expect("removed file is recoverable");

    assert_eq!(loaded, None);
}

/// When `cwd` is nested inside a repo, the search should locate AGENTS.md
/// placed at the repository root (identified by `.git`).
#[tokio::test]
async fn finds_doc_in_repo_root() {
    let repo = tempfile::tempdir().expect("tempdir");

    // Simulate a git repository. Note .git can be a file or a directory.
    std::fs::write(
        repo.path().join(".git"),
        "gitdir: /path/to/actual/git/dir\n",
    )
    .unwrap();

    // Put the doc at the repo root.
    fs::write(repo.path().join("AGENTS.md"), "root level doc").unwrap();

    // Now create a nested working directory: repo/workspace/crate_a
    let nested = repo.path().join("workspace/crate_a");
    std::fs::create_dir_all(&nested).unwrap();

    // Build config pointing at the nested dir.
    let mut cfg = make_config(&repo, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.cwd = nested.abs();

    let res = get_user_instructions(&cfg).await.expect("doc expected");
    assert_eq!(res, "root level doc");
}

/// Explicitly setting the byte-limit to zero disables project docs.
#[tokio::test]
async fn zero_byte_limit_disables_docs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "something").unwrap();

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 0, /*instructions*/ None).await).await;
    assert!(
        res.is_none(),
        "With limit 0 the function should return None"
    );
}

/// When both system instructions and AGENTS.md docs are present the two
/// should be concatenated with the separator.
#[tokio::test]
async fn merges_existing_instructions_with_agents_md() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "proj doc").unwrap();

    const INSTRUCTIONS: &str = "base instructions";

    let res = get_user_instructions(&make_config(&tmp, /*limit*/ 4096, Some(INSTRUCTIONS)).await)
        .await
        .expect("should produce a combined instruction string");

    let expected = format!("{INSTRUCTIONS}{AGENTS_MD_SEPARATOR}{}", "proj doc");

    assert_eq!(res, expected);
}

/// If there are existing system instructions but AGENTS.md docs are
/// missing we expect the original instructions to be returned unchanged.
#[tokio::test]
async fn keeps_existing_instructions_when_doc_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");

    const INSTRUCTIONS: &str = "some instructions";

    let res =
        get_user_instructions(&make_config(&tmp, /*limit*/ 4096, Some(INSTRUCTIONS)).await).await;

    assert_eq!(res, Some(INSTRUCTIONS.to_string()));
}

/// When both the repository root and the working directory contain
/// AGENTS.md files, their contents are concatenated from root to cwd.
#[tokio::test]
async fn concatenates_root_and_cwd_docs() {
    let repo = tempfile::tempdir().expect("tempdir");

    // Simulate a git repository.
    std::fs::write(
        repo.path().join(".git"),
        "gitdir: /path/to/actual/git/dir\n",
    )
    .unwrap();

    // Repo root doc.
    fs::write(repo.path().join("AGENTS.md"), "root doc").unwrap();

    // Nested working directory with its own doc.
    let nested = repo.path().join("workspace/crate_a");
    std::fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("AGENTS.md"), "crate doc").unwrap();

    let mut cfg = make_config(&repo, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.cwd = nested.abs();

    let mut warnings = Vec::new();
    let loaded = load_agents_md(&cfg, &mut warnings)
        .await
        .expect("doc expected");
    let root_agents = repo.path().join("AGENTS.md").abs();
    let crate_agents = cfg.cwd.join("AGENTS.md");
    let expected = LoadedAgentsMd {
        user_instructions: None,
        entries: vec![
            InstructionEntry {
                contents: "root doc".to_string(),
                provenance: InstructionProvenance::Project(root_agents.clone()),
            },
            InstructionEntry {
                contents: "crate doc".to_string(),
                provenance: InstructionProvenance::Project(crate_agents.clone()),
            },
        ],
    };

    assert_eq!(loaded, expected);
    assert_eq!(loaded.text(), "root doc\n\ncrate doc");
    assert_eq!(
        loaded.sources().collect::<Vec<_>>(),
        vec![&root_agents, &crate_agents]
    );
}

#[tokio::test]
async fn project_root_markers_are_honored_for_agents_discovery() {
    let root = tempfile::tempdir().expect("tempdir");
    fs::write(root.path().join(".codex-root"), "").unwrap();
    fs::write(root.path().join("AGENTS.md"), "parent doc").unwrap();

    let nested = root.path().join("dir1");
    fs::create_dir_all(nested.join(".git")).unwrap();
    fs::write(nested.join("AGENTS.md"), "child doc").unwrap();

    let mut cfg = make_config_with_project_root_markers(
        &root,
        /*limit*/ 4096,
        /*instructions*/ None,
        &[".codex-root"],
    )
    .await;
    cfg.cwd = nested.abs();

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    let expected_parent = root.path().join("AGENTS.md").abs();
    let expected_child = cfg.cwd.join("AGENTS.md");
    assert_eq!(discovery.len(), 2);
    assert_eq!(discovery[0], expected_parent);
    assert_eq!(discovery[1], expected_child);

    let res = get_user_instructions(&cfg).await.expect("doc expected");
    assert_eq!(res, "parent doc\n\nchild doc");
}

#[tokio::test]
async fn project_layers_do_not_override_project_root_markers() {
    let root = tempfile::tempdir().expect("tempdir");
    fs::write(root.path().join(".git"), "").unwrap();
    fs::write(root.path().join("AGENTS.md"), "root doc").unwrap();
    let nested = root.path().join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("AGENTS.md"), "nested doc").unwrap();

    let mut config = make_config(&root, /*limit*/ 4096, /*instructions*/ None).await;
    config.cwd = nested.abs();
    let project_layer = |dot_codex_folder: AbsolutePathBuf, marker: &str| {
        ConfigLayerEntry::new(
            ConfigLayerSource::Project { dot_codex_folder },
            TomlValue::Table(
                [(
                    "project_root_markers".to_string(),
                    TomlValue::Array(vec![TomlValue::String(marker.to_string())]),
                )]
                .into_iter()
                .collect(),
            ),
        )
    };
    config.config_layer_stack = ConfigLayerStack::new(
        vec![
            project_layer(root.path().join(".codex").abs(), ".ignored-root-marker"),
            project_layer(config.cwd.join(".codex"), ".ignored-nested-marker"),
        ],
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid project layer ordering");

    let discovery = agents_md_paths(&config).await.expect("discover paths");

    assert_eq!(
        discovery,
        vec![
            root.path().join("AGENTS.md").abs(),
            config.cwd.join("AGENTS.md"),
        ]
    );
}

#[tokio::test]
async fn agents_md_paths_preserve_symlinked_cwd() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("AGENTS.md"), "project doc").unwrap();

    let linked_cwd = tmp.path().join("linked");
    create_directory_symlink(&target, &linked_cwd);

    let mut cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.cwd = linked_cwd.abs();

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery, vec![cfg.cwd.join("AGENTS.md")]);

    let res = get_user_instructions(&cfg).await.expect("doc expected");
    assert_eq!(res, "project doc");
}

#[tokio::test]
async fn child_agents_message_after_global_instructions_uses_plain_separator() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = make_config(&tmp, /*limit*/ 4096, Some("global doc")).await;
    cfg.features.enable(Feature::ChildAgentsMd).unwrap();

    let mut warnings = Vec::new();
    let loaded = load_agents_md(&cfg, &mut warnings)
        .await
        .expect("instructions expected");
    let global_agents = cfg.codex_home.join(DEFAULT_AGENTS_MD_FILENAME);
    let expected = LoadedAgentsMd {
        user_instructions: Some(UserInstructions {
            text: "global doc".to_string(),
            source: global_agents,
        }),
        entries: vec![InstructionEntry {
            contents: HIERARCHICAL_AGENTS_MESSAGE.to_string(),
            provenance: InstructionProvenance::Internal,
        }],
    };

    assert_eq!(loaded, expected);
    assert_eq!(
        loaded.text(),
        format!("global doc\n\n{HIERARCHICAL_AGENTS_MESSAGE}")
    );
}

#[tokio::test]
async fn instruction_sources_include_global_before_agents_md_docs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "project doc").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, Some("global doc")).await;
    let global_agents = cfg.codex_home.join(DEFAULT_AGENTS_MD_FILENAME);
    fs::create_dir_all(&cfg.codex_home).unwrap();
    fs::write(&global_agents, "global doc").unwrap();

    let mut warnings = Vec::new();
    let loaded = load_agents_md(&cfg, &mut warnings)
        .await
        .expect("instructions expected");
    let project_agents = cfg.cwd.join("AGENTS.md");

    let expected = LoadedAgentsMd {
        user_instructions: Some(UserInstructions {
            text: "global doc".to_string(),
            source: global_agents.clone(),
        }),
        entries: vec![InstructionEntry {
            contents: "project doc".to_string(),
            provenance: InstructionProvenance::Project(project_agents.clone()),
        }],
    };
    assert_eq!(loaded, expected);
    assert_eq!(loaded.user_instructions(), cfg.user_instructions.as_ref());
    assert_eq!(
        loaded.sources().collect::<Vec<_>>(),
        vec![&global_agents, &project_agents]
    );
    assert_eq!(
        loaded.text(),
        format!("global doc{AGENTS_MD_SEPARATOR}project doc")
    );
}

#[tokio::test]
async fn child_agents_message_after_project_docs_is_not_an_instruction_source() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "project doc").unwrap();

    let mut cfg = make_config(&tmp, /*limit*/ 4096, Some("global doc")).await;
    cfg.features.enable(Feature::ChildAgentsMd).unwrap();
    let global_agents = cfg.codex_home.join(DEFAULT_AGENTS_MD_FILENAME);
    fs::create_dir_all(&cfg.codex_home).unwrap();
    fs::write(&global_agents, "global doc").unwrap();

    let mut warnings = Vec::new();
    let loaded = load_agents_md(&cfg, &mut warnings)
        .await
        .expect("instructions expected");
    let project_agents = cfg.cwd.join("AGENTS.md");

    let expected = LoadedAgentsMd {
        user_instructions: Some(UserInstructions {
            text: "global doc".to_string(),
            source: global_agents.clone(),
        }),
        entries: vec![
            InstructionEntry {
                contents: "project doc".to_string(),
                provenance: InstructionProvenance::Project(project_agents.clone()),
            },
            InstructionEntry {
                contents: HIERARCHICAL_AGENTS_MESSAGE.to_string(),
                provenance: InstructionProvenance::Internal,
            },
        ],
    };
    assert_eq!(loaded, expected);
    assert_eq!(
        loaded.sources().collect::<Vec<_>>(),
        vec![&global_agents, &project_agents]
    );
    assert_eq!(
        loaded.text(),
        format!("global doc{AGENTS_MD_SEPARATOR}project doc\n\n{HIERARCHICAL_AGENTS_MESSAGE}")
    );
}

/// AGENTS.override.md is preferred over AGENTS.md when both are present.
#[tokio::test]
async fn agents_local_md_preferred() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DEFAULT_AGENTS_MD_FILENAME), "versioned").unwrap();
    fs::write(tmp.path().join(LOCAL_AGENTS_MD_FILENAME), "local").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg)
        .await
        .expect("local doc expected");

    assert_eq!(res, "local");

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert_eq!(
        discovery[0].file_name().unwrap().to_string_lossy(),
        LOCAL_AGENTS_MD_FILENAME
    );
}

/// When AGENTS.md is absent but a configured fallback exists, the fallback is used.
#[tokio::test]
async fn uses_configured_fallback_when_agents_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("EXAMPLE.md"), "example instructions").unwrap();

    let cfg = make_config_with_fallback(
        &tmp,
        /*limit*/ 4096,
        /*instructions*/ None,
        &["EXAMPLE.md"],
    )
    .await;

    let res = get_user_instructions(&cfg)
        .await
        .expect("fallback doc expected");

    assert_eq!(res, "example instructions");
}

/// AGENTS.md remains preferred when both AGENTS.md and fallbacks are present.
#[tokio::test]
async fn agents_md_preferred_over_fallbacks() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "primary").unwrap();
    fs::write(tmp.path().join("EXAMPLE.md"), "secondary").unwrap();

    let cfg = make_config_with_fallback(
        &tmp,
        /*limit*/ 4096,
        /*instructions*/ None,
        &["EXAMPLE.md", ".example.md"],
    )
    .await;

    let res = get_user_instructions(&cfg)
        .await
        .expect("AGENTS.md should win");

    assert_eq!(res, "primary");

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert!(
        discovery[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .eq(DEFAULT_AGENTS_MD_FILENAME)
    );
}

#[tokio::test]
async fn agents_md_directory_is_ignored() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(tmp.path().join("AGENTS.md")).unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg).await;
    assert_eq!(res, None);

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery, Vec::<AbsolutePathBuf>::new());
}

#[cfg(unix)]
#[tokio::test]
async fn agents_md_special_file_is_ignored() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().join("AGENTS.md");
    let c_path = CString::new(path.as_os_str().as_bytes()).expect("path without nul");
    // SAFETY: `c_path` is a valid, nul-terminated path and `mkfifo` does not
    // retain the pointer after the call.
    let rc = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };
    assert_eq!(rc, 0);

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg).await;
    assert_eq!(res, None);

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery, Vec::<AbsolutePathBuf>::new());
}

#[tokio::test]
async fn override_directory_falls_back_to_agents_md_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(tmp.path().join(LOCAL_AGENTS_MD_FILENAME)).unwrap();
    fs::write(tmp.path().join(DEFAULT_AGENTS_MD_FILENAME), "primary").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;

    let res = get_user_instructions(&cfg)
        .await
        .expect("AGENTS.md should be used when override is a directory");
    assert_eq!(res, "primary");

    let discovery = agents_md_paths(&cfg).await.expect("discover paths");
    assert_eq!(discovery.len(), 1);
    assert_eq!(
        discovery[0]
            .file_name()
            .expect("file name")
            .to_string_lossy(),
        DEFAULT_AGENTS_MD_FILENAME
    );
}

#[tokio::test]
async fn skills_are_not_appended_to_agents_md() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "base doc").unwrap();

    let cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    create_skill(
        cfg.codex_home.to_path_buf(),
        "pdf-processing",
        "extract from pdfs",
    );

    let res = get_user_instructions(&cfg)
        .await
        .expect("instructions expected");
    assert_eq!(res, "base doc");
}

#[tokio::test]
async fn apps_feature_does_not_emit_user_instructions_by_itself() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.features
        .enable(Feature::Apps)
        .expect("test config should allow apps");

    let res = get_user_instructions(&cfg).await;
    assert_eq!(res, None);
}

#[tokio::test]
async fn apps_feature_does_not_append_to_agents_md_user_instructions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join("AGENTS.md"), "base doc").unwrap();

    let mut cfg = make_config(&tmp, /*limit*/ 4096, /*instructions*/ None).await;
    cfg.features
        .enable(Feature::Apps)
        .expect("test config should allow apps");

    let res = get_user_instructions(&cfg)
        .await
        .expect("instructions expected");
    assert_eq!(res, "base doc");
}

fn create_skill(codex_home: PathBuf, name: &str, description: &str) {
    let skill_dir = codex_home.join(format!("skills/{name}"));
    fs::create_dir_all(&skill_dir).unwrap();
    let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
}
