use assert_matches::assert_matches;
use async_trait::async_trait;
use codex_apply_patch::MaybeApplyPatchVerified;
use codex_apply_patch::apply_patch;
use codex_apply_patch::maybe_parse_apply_patch_verified;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemResult;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::SandboxPolicy;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::tempdir;
use tokio::io;

#[derive(Debug, Clone, PartialEq, Eq)]
enum FsCall {
    ReadFile {
        path: PathBuf,
        sandbox: Option<FileSystemSandboxContext>,
    },
    WriteFile {
        path: PathBuf,
        sandbox: Option<FileSystemSandboxContext>,
    },
    CreateDirectory {
        path: PathBuf,
        sandbox: Option<FileSystemSandboxContext>,
    },
    GetMetadata {
        path: PathBuf,
        sandbox: Option<FileSystemSandboxContext>,
    },
    Remove {
        path: PathBuf,
        sandbox: Option<FileSystemSandboxContext>,
    },
}

#[derive(Default)]
struct RecordingFileSystem {
    calls: Mutex<Vec<FsCall>>,
    files: Mutex<HashMap<PathBuf, Vec<u8>>>,
}

impl RecordingFileSystem {
    fn with_file(path: PathBuf, contents: &str) -> Self {
        let mut files = HashMap::new();
        files.insert(path, contents.as_bytes().to_vec());
        Self {
            calls: Mutex::new(Vec::new()),
            files: Mutex::new(files),
        }
    }

    fn calls(&self) -> Vec<FsCall> {
        self.calls
            .lock()
            .unwrap_or_else(|err| panic!("calls lock poisoned: {err}"))
            .clone()
    }
}

#[async_trait]
impl ExecutorFileSystem for RecordingFileSystem {
    async fn read_file(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<u8>> {
        self.calls
            .lock()
            .unwrap_or_else(|err| panic!("calls lock poisoned: {err}"))
            .push(FsCall::ReadFile {
                path: path.to_path_buf(),
                sandbox: sandbox.cloned(),
            });
        self.files
            .lock()
            .unwrap_or_else(|err| panic!("files lock poisoned: {err}"))
            .get(path.as_path())
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "missing test file"))
    }

    async fn write_file(
        &self,
        path: &AbsolutePathBuf,
        contents: Vec<u8>,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        self.calls
            .lock()
            .unwrap_or_else(|err| panic!("calls lock poisoned: {err}"))
            .push(FsCall::WriteFile {
                path: path.to_path_buf(),
                sandbox: sandbox.cloned(),
            });
        self.files
            .lock()
            .unwrap_or_else(|err| panic!("files lock poisoned: {err}"))
            .insert(path.to_path_buf(), contents);
        Ok(())
    }

    async fn create_directory(
        &self,
        path: &AbsolutePathBuf,
        _create_directory_options: CreateDirectoryOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        self.calls
            .lock()
            .unwrap_or_else(|err| panic!("calls lock poisoned: {err}"))
            .push(FsCall::CreateDirectory {
                path: path.to_path_buf(),
                sandbox: sandbox.cloned(),
            });
        Ok(())
    }

    async fn get_metadata(
        &self,
        path: &AbsolutePathBuf,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<FileMetadata> {
        self.calls
            .lock()
            .unwrap_or_else(|err| panic!("calls lock poisoned: {err}"))
            .push(FsCall::GetMetadata {
                path: path.to_path_buf(),
                sandbox: sandbox.cloned(),
            });
        let is_file = self
            .files
            .lock()
            .unwrap_or_else(|err| panic!("files lock poisoned: {err}"))
            .contains_key(path.as_path());
        Ok(FileMetadata {
            is_directory: false,
            is_file,
            created_at_ms: 0,
            modified_at_ms: 0,
        })
    }

    async fn read_directory(
        &self,
        _path: &AbsolutePathBuf,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<Vec<ReadDirectoryEntry>> {
        unimplemented!("read_directory should not be used in fs_sandbox tests")
    }

    async fn remove(
        &self,
        path: &AbsolutePathBuf,
        _remove_options: RemoveOptions,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        self.calls
            .lock()
            .unwrap_or_else(|err| panic!("calls lock poisoned: {err}"))
            .push(FsCall::Remove {
                path: path.to_path_buf(),
                sandbox: sandbox.cloned(),
            });
        self.files
            .lock()
            .unwrap_or_else(|err| panic!("files lock poisoned: {err}"))
            .remove(path.as_path());
        Ok(())
    }

    async fn copy(
        &self,
        _source_path: &AbsolutePathBuf,
        _destination_path: &AbsolutePathBuf,
        _copy_options: CopyOptions,
        _sandbox: Option<&FileSystemSandboxContext>,
    ) -> FileSystemResult<()> {
        unimplemented!("copy should not be used in fs_sandbox tests")
    }
}

fn test_sandbox_context() -> FileSystemSandboxContext {
    FileSystemSandboxContext {
        sandbox_policy: SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: Default::default(),
            network_access: false,
            exclude_tmpdir_env_var: true,
            exclude_slash_tmp: true,
        },
        windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
        windows_sandbox_private_desktop: true,
        use_legacy_landlock: true,
        additional_permissions: None,
    }
}

fn wrap_patch(body: &str) -> String {
    format!("*** Begin Patch\n{body}\n*** End Patch")
}

#[tokio::test]
async fn verified_parse_passes_sandbox_to_filesystem_reads() {
    let dir = tempdir().unwrap_or_else(|err| panic!("tempdir failed: {err}"));
    let path = dir.path().join("source.txt");
    let path_abs = AbsolutePathBuf::from_absolute_path(&path)
        .unwrap_or_else(|err| panic!("absolute file path failed: {err}"));
    let fs = RecordingFileSystem::with_file(path.clone(), "before\n");
    let sandbox = test_sandbox_context();
    let argv = vec![
        "apply_patch".to_string(),
        wrap_patch(
            r#"*** Update File: source.txt
@@
-before
+after"#,
        ),
    ];

    let result = maybe_parse_apply_patch_verified(
        &argv,
        &AbsolutePathBuf::from_absolute_path(dir.path())
            .unwrap_or_else(|err| panic!("absolute cwd failed: {err}")),
        &fs,
        Some(&sandbox),
    )
    .await;

    assert_matches!(result, MaybeApplyPatchVerified::Body(_));
    assert_eq!(
        fs.calls(),
        vec![FsCall::ReadFile {
            path: path_abs.to_path_buf(),
            sandbox: Some(sandbox),
        }]
    );
}

#[tokio::test]
async fn apply_patch_passes_sandbox_to_add_file_operations() {
    let dir = tempdir().unwrap_or_else(|err| panic!("tempdir failed: {err}"));
    let cwd = AbsolutePathBuf::from_absolute_path(dir.path())
        .unwrap_or_else(|err| panic!("absolute cwd failed: {err}"));
    let path_abs = cwd.join("nested/add.txt");
    let parent_abs = path_abs
        .parent()
        .unwrap_or_else(|| panic!("expected parent for {}", path_abs.display()));
    let fs = RecordingFileSystem::default();
    let sandbox = test_sandbox_context();
    let patch = wrap_patch(
        r#"*** Add File: nested/add.txt
+hello"#,
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    apply_patch(&patch, &cwd, &mut stdout, &mut stderr, &fs, Some(&sandbox))
        .await
        .unwrap_or_else(|err| panic!("apply patch failed: {err}"));

    assert_eq!(
        fs.calls(),
        vec![
            FsCall::CreateDirectory {
                path: parent_abs.to_path_buf(),
                sandbox: Some(sandbox.clone()),
            },
            FsCall::WriteFile {
                path: path_abs.to_path_buf(),
                sandbox: Some(sandbox),
            },
        ]
    );
}

#[tokio::test]
async fn apply_patch_passes_sandbox_to_delete_file_operations() {
    let dir = tempdir().unwrap_or_else(|err| panic!("tempdir failed: {err}"));
    let cwd = AbsolutePathBuf::from_absolute_path(dir.path())
        .unwrap_or_else(|err| panic!("absolute cwd failed: {err}"));
    let path_abs = cwd.join("del.txt");
    let fs = RecordingFileSystem::with_file(path_abs.to_path_buf(), "before\n");
    let sandbox = test_sandbox_context();
    let patch = wrap_patch("*** Delete File: del.txt");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    apply_patch(&patch, &cwd, &mut stdout, &mut stderr, &fs, Some(&sandbox))
        .await
        .unwrap_or_else(|err| panic!("apply patch failed: {err}"));

    assert_eq!(
        fs.calls(),
        vec![
            FsCall::GetMetadata {
                path: path_abs.to_path_buf(),
                sandbox: Some(sandbox.clone()),
            },
            FsCall::Remove {
                path: path_abs.to_path_buf(),
                sandbox: Some(sandbox),
            },
        ]
    );
}

#[tokio::test]
async fn apply_patch_passes_sandbox_to_update_file_operations() {
    let dir = tempdir().unwrap_or_else(|err| panic!("tempdir failed: {err}"));
    let cwd = AbsolutePathBuf::from_absolute_path(dir.path())
        .unwrap_or_else(|err| panic!("absolute cwd failed: {err}"));
    let path_abs = cwd.join("update.txt");
    let fs = RecordingFileSystem::with_file(path_abs.to_path_buf(), "before\n");
    let sandbox = test_sandbox_context();
    let patch = wrap_patch(
        r#"*** Update File: update.txt
@@
-before
+after"#,
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    apply_patch(&patch, &cwd, &mut stdout, &mut stderr, &fs, Some(&sandbox))
        .await
        .unwrap_or_else(|err| panic!("apply patch failed: {err}"));

    assert_eq!(
        fs.calls(),
        vec![
            FsCall::ReadFile {
                path: path_abs.to_path_buf(),
                sandbox: Some(sandbox.clone()),
            },
            FsCall::WriteFile {
                path: path_abs.to_path_buf(),
                sandbox: Some(sandbox),
            },
        ]
    );
}
