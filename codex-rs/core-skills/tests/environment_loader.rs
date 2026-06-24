use std::fs;
use std::sync::Mutex;

use codex_core_skills::loader::EnvironmentSkillMetadata;
use codex_core_skills::loader::load_environment_skills_from_root;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LOCAL_FS;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::RemoveOptions;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

struct RecordingFileSystem<'a> {
    inner: &'a dyn ExecutorFileSystem,
    read_files: Mutex<Vec<PathUri>>,
}

impl<'a> RecordingFileSystem<'a> {
    fn new(inner: &'a dyn ExecutorFileSystem) -> Self {
        Self {
            inner,
            read_files: Mutex::new(Vec::new()),
        }
    }

    fn read_files(&self) -> Vec<PathUri> {
        self.read_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ExecutorFileSystem for RecordingFileSystem<'_> {
    fn canonicalize<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, PathUri> {
        self.inner.canonicalize(path, sandbox)
    }

    fn read_file<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        self.read_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
        self.inner.read_file(path, sandbox)
    }

    fn read_file_stream<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileSystemReadStream> {
        self.inner.read_file_stream(path, sandbox)
    }

    fn write_file<'a>(
        &'a self,
        path: &'a PathUri,
        contents: Vec<u8>,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.write_file(path, contents, sandbox)
    }

    fn create_directory<'a>(
        &'a self,
        path: &'a PathUri,
        options: CreateDirectoryOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.create_directory(path, options, sandbox)
    }

    fn get_metadata<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        self.inner.get_metadata(path, sandbox)
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        self.inner.read_directory(path, sandbox)
    }

    fn remove<'a>(
        &'a self,
        path: &'a PathUri,
        options: RemoveOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.remove(path, options, sandbox)
    }

    fn copy<'a>(
        &'a self,
        source_path: &'a PathUri,
        destination_path: &'a PathUri,
        options: CopyOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner
            .copy(source_path, destination_path, options, sandbox)
    }
}

#[tokio::test]
async fn loads_nearest_plugin_namespaces_without_reading_unused_sibling_manifests() {
    let root = tempdir().expect("tempdir");
    let standalone_skill = root.path().join("standalone/SKILL.md");
    let outer_root = root.path().join("plugins/outer");
    let outer_skill = outer_root.join("skills/deploy/SKILL.md");
    let inner_root = outer_root.join("nested/inner");
    let inner_skill = inner_root.join("skills/audit/SKILL.md");
    let unused_root = root.path().join("plugins/unused");

    for path in [&standalone_skill, &outer_skill, &inner_skill] {
        fs::create_dir_all(path.parent().expect("skill parent")).expect("skill dir");
    }
    for (plugin_root, name) in [
        (&outer_root, "outer"),
        (&inner_root, "inner"),
        (&unused_root, "unused"),
    ] {
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("manifest dir");
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            format!(r#"{{"name":"{name}"}}"#),
        )
        .expect("manifest");
    }
    for (path, name) in [
        (&standalone_skill, "standalone"),
        (&outer_skill, "deploy"),
        (&inner_skill, "audit"),
    ] {
        fs::write(
            path,
            format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
        )
        .expect("skill");
    }

    let file_system = RecordingFileSystem::new(LOCAL_FS.as_ref());
    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome = load_environment_skills_from_root(
        &file_system,
        &root_uri,
        /*restriction_product*/ None,
    )
    .await;

    assert_eq!(outcome.warnings, Vec::<String>::new());
    assert_eq!(
        outcome.skills,
        vec![
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&inner_skill).unwrap(),
                name: "inner:audit".to_string(),
                description: "audit skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&outer_skill).unwrap(),
                name: "outer:deploy".to_string(),
                description: "deploy skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
            EnvironmentSkillMetadata {
                path_to_skills_md: PathUri::from_host_native_path(&standalone_skill).unwrap(),
                name: "standalone".to_string(),
                description: "standalone skill.".to_string(),
                short_description: None,
                dependencies: None,
                policy: None,
            },
        ]
    );

    let mut manifest_reads = file_system
        .read_files()
        .into_iter()
        .filter(|path| path.basename().as_deref() == Some("plugin.json"))
        .collect::<Vec<_>>();
    manifest_reads.sort_by_key(ToString::to_string);
    let mut expected_manifest_reads = [&outer_root, &inner_root]
        .into_iter()
        .map(|plugin_root| {
            PathUri::from_host_native_path(plugin_root.join(".codex-plugin/plugin.json")).unwrap()
        })
        .collect::<Vec<_>>();
    expected_manifest_reads.sort_by_key(ToString::to_string);
    assert_eq!(manifest_reads, expected_manifest_reads);
}
