use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::io;
use std::path::Path;
use std::sync::Arc;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::ExecutorFileSystem;
use crate::FileMetadata;
use crate::FileSystemSandboxContext;
use crate::LOCAL_FS;
use crate::ReadDirectoryEntry;

/// Binds an absolute path to the executor filesystem that owns it.
#[derive(Clone)]
pub struct EnvironmentPathRef {
    file_system: Arc<dyn ExecutorFileSystem>,
    path: AbsolutePathBuf,
}

impl EnvironmentPathRef {
    /// Creates a path ref bound to the filesystem that owns `path`.
    pub fn new(file_system: Arc<dyn ExecutorFileSystem>, path: AbsolutePathBuf) -> Self {
        Self { file_system, path }
    }

    /// Creates a path ref bound to the shared unsandboxed local filesystem.
    pub fn local(path: AbsolutePathBuf) -> Self {
        Self::new(Arc::clone(&LOCAL_FS), path)
    }

    /// Returns the absolute path held by this ref.
    pub fn path(&self) -> &AbsolutePathBuf {
        &self.path
    }

    /// Returns the filesystem that owns this path.
    pub fn file_system(&self) -> Arc<dyn ExecutorFileSystem> {
        Arc::clone(&self.file_system)
    }

    /// Reads this path as UTF-8 text through its bound filesystem.
    pub async fn read_to_string(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<String> {
        self.file_system.read_file_text(&self.path, sandbox).await
    }

    /// Reads metadata for this path through its bound filesystem.
    pub async fn metadata(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<FileMetadata> {
        self.file_system.get_metadata(&self.path, sandbox).await
    }

    /// Reads directory entries for this path through its bound filesystem.
    pub async fn read_directory(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Vec<ReadDirectoryEntry>> {
        self.file_system.read_directory(&self.path, sandbox).await
    }

    /// Returns a ref with the same filesystem and a replacement path.
    pub fn with_path(&self, path: AbsolutePathBuf) -> Self {
        Self::new(Arc::clone(&self.file_system), path)
    }

    /// Lexically joins `path` onto this path through its bound filesystem.
    pub async fn join<P: AsRef<Path>>(&self, path: P) -> io::Result<Self> {
        self.file_system
            .join(&self.path, path.as_ref())
            .await
            .map(|path| self.with_path(path))
    }

    /// Returns the parent of this path through its bound filesystem.
    pub async fn parent(&self) -> io::Result<Option<Self>> {
        self.file_system
            .parent(&self.path)
            .await
            .map(|path| path.map(|path| self.with_path(path)))
    }

    /// Canonicalizes this path through its bound filesystem.
    pub async fn canonicalize(
        &self,
        sandbox: Option<&FileSystemSandboxContext>,
    ) -> io::Result<Self> {
        self.file_system
            .canonicalize(&self.path, sandbox)
            .await
            .map(|path| self.with_path(path))
    }
}

impl PartialEq for EnvironmentPathRef {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.file_system, &other.file_system) && self.path == other.path
    }
}

impl Eq for EnvironmentPathRef {}

impl Hash for EnvironmentPathRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.file_system) as *const () as usize).hash(state);
        self.path.hash(state);
    }
}

impl fmt::Debug for EnvironmentPathRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvironmentPathRef")
            .field("path", &self.path)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_trait::async_trait;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;
    use std::path::Path;
    use std::sync::Mutex;

    use crate::LOCAL_FS;

    #[derive(Clone, Debug, Eq, PartialEq)]
    enum RecordedMethod {
        Canonicalize,
        Join,
        Parent,
        ReadFileText,
        Metadata,
        ReadDirectory,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct RecordedCall {
        method: RecordedMethod,
        path: AbsolutePathBuf,
        sandbox: Option<FileSystemSandboxContext>,
    }

    struct RecordingFileSystem {
        calls: Mutex<Vec<RecordedCall>>,
    }

    impl Default for RecordingFileSystem {
        fn default() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl RecordingFileSystem {
        fn recorded_calls(&self) -> Vec<RecordedCall> {
            match self.calls.lock() {
                Ok(calls) => calls.clone(),
                Err(err) => err.into_inner().clone(),
            }
        }

        fn push_call(&self, call: RecordedCall) {
            match self.calls.lock() {
                Ok(mut calls) => calls.push(call),
                Err(err) => err.into_inner().push(call),
            }
        }
    }

    fn local_path_ref(path: AbsolutePathBuf) -> EnvironmentPathRef {
        EnvironmentPathRef::new(Arc::clone(&LOCAL_FS), path)
    }

    #[async_trait]
    impl ExecutorFileSystem for RecordingFileSystem {
        async fn canonicalize(
            &self,
            path: &AbsolutePathBuf,
            _sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<AbsolutePathBuf> {
            self.push_call(RecordedCall {
                method: RecordedMethod::Canonicalize,
                path: path.clone(),
                sandbox: None,
            });
            Ok(path.parent().unwrap())
        }

        async fn join(
            &self,
            base_path: &AbsolutePathBuf,
            path: &Path,
        ) -> io::Result<AbsolutePathBuf> {
            self.push_call(RecordedCall {
                method: RecordedMethod::Join,
                path: base_path.clone(),
                sandbox: None,
            });
            AbsolutePathBuf::from_absolute_path_checked(base_path.as_path().join(path))
        }

        async fn parent(&self, path: &AbsolutePathBuf) -> io::Result<Option<AbsolutePathBuf>> {
            self.push_call(RecordedCall {
                method: RecordedMethod::Parent,
                path: path.clone(),
                sandbox: None,
            });
            Ok(path.parent())
        }

        async fn read_file(
            &self,
            path: &AbsolutePathBuf,
            sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<Vec<u8>> {
            self.push_call(RecordedCall {
                method: RecordedMethod::ReadFileText,
                path: path.clone(),
                sandbox: sandbox.cloned(),
            });
            Ok(b"skill contents".to_vec())
        }

        async fn write_file(
            &self,
            _path: &AbsolutePathBuf,
            _contents: Vec<u8>,
            _sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<()> {
            unreachable!("write_file should not be called")
        }

        async fn create_directory(
            &self,
            _path: &AbsolutePathBuf,
            _create_directory_options: crate::CreateDirectoryOptions,
            _sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<()> {
            unreachable!("create_directory should not be called")
        }

        async fn get_metadata(
            &self,
            path: &AbsolutePathBuf,
            sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<FileMetadata> {
            self.push_call(RecordedCall {
                method: RecordedMethod::Metadata,
                path: path.clone(),
                sandbox: sandbox.cloned(),
            });
            Ok(FileMetadata {
                is_directory: true,
                is_file: false,
                is_symlink: false,
                created_at_ms: 1,
                modified_at_ms: 2,
            })
        }

        async fn read_directory(
            &self,
            path: &AbsolutePathBuf,
            sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<Vec<ReadDirectoryEntry>> {
            self.push_call(RecordedCall {
                method: RecordedMethod::ReadDirectory,
                path: path.clone(),
                sandbox: sandbox.cloned(),
            });
            Ok(vec![ReadDirectoryEntry {
                file_name: "SKILL.md".to_string(),
                is_directory: false,
                is_file: true,
            }])
        }

        async fn remove(
            &self,
            _path: &AbsolutePathBuf,
            _remove_options: crate::RemoveOptions,
            _sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<()> {
            unreachable!("remove should not be called")
        }

        async fn copy(
            &self,
            _source_path: &AbsolutePathBuf,
            _destination_path: &AbsolutePathBuf,
            _copy_options: crate::CopyOptions,
            _sandbox: Option<&FileSystemSandboxContext>,
        ) -> io::Result<()> {
            unreachable!("copy should not be called")
        }
    }

    fn restricted_sandbox() -> FileSystemSandboxContext {
        FileSystemSandboxContext::from_permission_profile(
            PermissionProfile::from_runtime_permissions(
                &FileSystemSandboxPolicy::restricted(Vec::new()),
                NetworkSandboxPolicy::Restricted,
            ),
        )
    }

    #[tokio::test]
    async fn environment_path_ref_forwards_sandbox_to_file_system_methods() {
        let path = std::env::temp_dir().join("skills/demo").abs();
        let file_system = Arc::new(RecordingFileSystem::default());
        let path_ref = EnvironmentPathRef::new(file_system.clone(), path.clone());
        let sandbox = restricted_sandbox();

        assert_eq!(
            path_ref
                .read_to_string(Some(&sandbox))
                .await
                .expect("read skill contents"),
            "skill contents".to_string()
        );
        assert_eq!(
            path_ref
                .metadata(Some(&sandbox))
                .await
                .expect("read metadata"),
            FileMetadata {
                is_directory: true,
                is_file: false,
                is_symlink: false,
                created_at_ms: 1,
                modified_at_ms: 2,
            }
        );
        assert_eq!(
            path_ref
                .read_directory(Some(&sandbox))
                .await
                .expect("read directory"),
            vec![ReadDirectoryEntry {
                file_name: "SKILL.md".to_string(),
                is_directory: false,
                is_file: true,
            }]
        );
        assert_eq!(
            file_system.recorded_calls(),
            vec![
                RecordedCall {
                    method: RecordedMethod::ReadFileText,
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                },
                RecordedCall {
                    method: RecordedMethod::Metadata,
                    path: path.clone(),
                    sandbox: Some(sandbox.clone()),
                },
                RecordedCall {
                    method: RecordedMethod::ReadDirectory,
                    path,
                    sandbox: Some(sandbox),
                },
            ]
        );
    }

    #[test]
    fn environment_path_ref_equality_and_hash_include_file_system_identity() {
        let path = std::env::temp_dir().join("skills/demo").abs();
        let file_system = Arc::new(RecordingFileSystem::default());
        let same_file_system: Arc<dyn ExecutorFileSystem> = file_system.clone();
        let different_file_system: Arc<dyn ExecutorFileSystem> =
            Arc::new(RecordingFileSystem::default());

        let left = EnvironmentPathRef::new(same_file_system.clone(), path.clone());
        let same = EnvironmentPathRef::new(same_file_system, path.clone());
        let different_path = EnvironmentPathRef::new(file_system, path.parent().unwrap());
        let different_fs = EnvironmentPathRef::new(different_file_system, path);

        assert_eq!(left, same);
        assert_ne!(left, different_path);
        assert_ne!(left, different_fs);

        let set = HashSet::from([left, same, different_path, different_fs]);
        assert_eq!(set.len(), 3);
    }
    #[tokio::test]
    async fn canonicalize_keeps_bound_file_system_identity() {
        let path = std::env::temp_dir().join("skills/demo").abs();
        let file_system = Arc::new(RecordingFileSystem::default());
        let path_ref = EnvironmentPathRef::new(file_system.clone(), path.clone());

        let canonicalized = path_ref
            .canonicalize(/*sandbox*/ None)
            .await
            .expect("canonicalize");

        assert_eq!(canonicalized.path(), &path.parent().unwrap());
        assert_eq!(
            canonicalized,
            EnvironmentPathRef::new(file_system.clone(), path.parent().unwrap())
        );
        assert_eq!(
            file_system.recorded_calls(),
            vec![RecordedCall {
                method: RecordedMethod::Canonicalize,
                path,
                sandbox: None,
            }]
        );
    }

    #[tokio::test]
    async fn join_keeps_bound_file_system_identity() {
        let path = std::env::temp_dir().join("skills").abs();
        let file_system = Arc::new(RecordingFileSystem::default());
        let path_ref = EnvironmentPathRef::new(file_system.clone(), path.clone());

        assert_eq!(
            path_ref.join(Path::new("demo")).await.ok(),
            Some(EnvironmentPathRef::new(
                file_system.clone(),
                std::env::temp_dir().join("skills/demo").abs(),
            ))
        );
        assert_eq!(
            file_system.recorded_calls(),
            vec![RecordedCall {
                method: RecordedMethod::Join,
                path,
                sandbox: None,
            }]
        );
    }

    #[tokio::test]
    async fn join_matches_absolute_path_buf_for_tilde_paths() {
        let path_ref = local_path_ref(std::env::temp_dir().join("skills").abs());

        assert_eq!(
            path_ref
                .join(Path::new("~"))
                .await
                .ok()
                .map(|path_ref| path_ref.path().clone()),
            Some(path_ref.path().join(Path::new("~")))
        );
    }

    #[tokio::test]
    async fn join_matches_absolute_path_buf_for_parent_dirs() {
        let path_ref = local_path_ref(std::env::temp_dir().join("skills").abs());

        assert_eq!(
            path_ref
                .join(Path::new("../outside"))
                .await
                .expect("join")
                .path()
                .clone(),
            path_ref.path().join(Path::new("../outside"))
        );
    }

    #[tokio::test]
    async fn parent_keeps_bound_file_system_identity() {
        let path = std::env::temp_dir().join("skills/demo").abs();
        let file_system = Arc::new(RecordingFileSystem::default());
        let path_ref = EnvironmentPathRef::new(file_system.clone(), path.clone());

        assert_eq!(
            path_ref.parent().await.expect("parent"),
            Some(EnvironmentPathRef::new(
                file_system.clone(),
                std::env::temp_dir().join("skills").abs(),
            ))
        );
        assert_eq!(
            file_system.recorded_calls(),
            vec![RecordedCall {
                method: RecordedMethod::Parent,
                path,
                sandbox: None,
            }]
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn join_matches_absolute_path_buf_for_windows_prefixed_and_rooted_paths() {
        let path_ref = local_path_ref(std::env::temp_dir().join("skills").abs());

        assert_eq!(
            path_ref
                .join(Path::new(r"C:temp"))
                .await
                .expect("join")
                .path()
                .clone(),
            path_ref.path().join(Path::new(r"C:temp"))
        );
        assert_eq!(
            path_ref
                .join(Path::new(r"\temp"))
                .await
                .expect("join")
                .path()
                .clone(),
            path_ref.path().join(Path::new(r"\temp"))
        );
    }
}
