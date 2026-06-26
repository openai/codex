use std::fs;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

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
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use tokio::sync::Notify;

#[derive(Clone, Copy)]
enum ManifestMetadataBehavior {
    Immediate,
    WaitForSkillRead,
}

struct RecordingFileSystem<'a> {
    inner: &'a dyn ExecutorFileSystem,
    read_files: Mutex<Vec<PathUri>>,
    metadata_files: Mutex<Vec<PathUri>>,
    walks: AtomicUsize,
    manifest_metadata_behavior: ManifestMetadataBehavior,
    skill_read_started: AtomicBool,
    skill_read_started_notify: Notify,
}

#[derive(Debug, PartialEq, Eq)]
struct FileSystemCalls {
    walks: usize,
    read_files: Vec<PathUri>,
    metadata_files: Vec<PathUri>,
}

impl<'a> RecordingFileSystem<'a> {
    fn new(
        inner: &'a dyn ExecutorFileSystem,
        manifest_metadata_behavior: ManifestMetadataBehavior,
    ) -> Self {
        Self {
            inner,
            read_files: Mutex::new(Vec::new()),
            metadata_files: Mutex::new(Vec::new()),
            walks: AtomicUsize::new(0),
            manifest_metadata_behavior,
            skill_read_started: AtomicBool::new(false),
            skill_read_started_notify: Notify::new(),
        }
    }

    fn calls(&self) -> FileSystemCalls {
        let mut read_files = self
            .read_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        read_files.sort_by_key(ToString::to_string);
        let mut metadata_files = self
            .metadata_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        metadata_files.sort_by_key(ToString::to_string);
        FileSystemCalls {
            walks: self.walks.load(Ordering::Relaxed),
            read_files,
            metadata_files,
        }
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
        if path.basename().as_deref() == Some("SKILL.md") {
            self.skill_read_started.store(true, Ordering::Release);
            self.skill_read_started_notify.notify_waiters();
        }
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
        self.metadata_files
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(path.clone());
        if matches!(
            self.manifest_metadata_behavior,
            ManifestMetadataBehavior::WaitForSkillRead
        ) && path.basename().as_deref() == Some("plugin.json")
        {
            return Box::pin(async move {
                loop {
                    let notified = self.skill_read_started_notify.notified();
                    if self.skill_read_started.load(Ordering::Acquire) {
                        break;
                    }
                    notified.await;
                }
                self.inner.get_metadata(path, sandbox).await
            });
        }
        self.inner.get_metadata(path, sandbox)
    }

    fn read_directory<'a>(
        &'a self,
        path: &'a PathUri,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<ReadDirectoryEntry>> {
        self.inner.read_directory(path, sandbox)
    }

    fn walk<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
        self.walks.fetch_add(1, Ordering::Relaxed);
        self.inner.walk(path, options, sandbox)
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

    let file_system =
        RecordingFileSystem::new(LOCAL_FS.as_ref(), ManifestMetadataBehavior::Immediate);
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
        .calls()
        .read_files
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

#[tokio::test]
async fn reuses_walk_inventory_for_missing_skill_metadata() {
    const SKILL_COUNT: usize = 66;

    let root = tempdir().expect("tempdir");
    let manifest_path = root.path().join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent")).expect("manifest dir");
    fs::write(&manifest_path, r#"{"name":"inventory"}"#).expect("manifest");

    let mut skill_paths = Vec::new();
    for index in 0..SKILL_COUNT {
        let name = format!("skill-{index}");
        let skill_path = root.path().join(&name).join("SKILL.md");
        fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill dir");
        fs::write(
            &skill_path,
            format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
        )
        .expect("skill");
        skill_paths.push(skill_path);
    }

    let file_system =
        RecordingFileSystem::new(LOCAL_FS.as_ref(), ManifestMetadataBehavior::Immediate);
    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome = load_environment_skills_from_root(
        &file_system,
        &root_uri,
        /*restriction_product*/ None,
    )
    .await;

    let mut expected_skills = skill_paths
        .iter()
        .enumerate()
        .map(|(index, skill_path)| EnvironmentSkillMetadata {
            path_to_skills_md: PathUri::from_host_native_path(skill_path).unwrap(),
            name: format!("inventory:skill-{index}"),
            description: format!("skill-{index} skill."),
            short_description: None,
            dependencies: None,
            policy: None,
        })
        .collect::<Vec<_>>();
    expected_skills.sort_by(|left, right| {
        left.name.cmp(&right.name).then_with(|| {
            left.path_to_skills_md
                .to_string()
                .cmp(&right.path_to_skills_md.to_string())
        })
    });
    assert_eq!(outcome.skills, expected_skills);
    assert_eq!(outcome.warnings, Vec::<String>::new());

    let mut expected_read_files = skill_paths
        .iter()
        .map(|path| PathUri::from_host_native_path(path).unwrap())
        .collect::<Vec<_>>();
    let manifest_uri = PathUri::from_host_native_path(manifest_path).unwrap();
    expected_read_files.push(manifest_uri.clone());
    expected_read_files.sort_by_key(ToString::to_string);
    assert_eq!(
        file_system.calls(),
        FileSystemCalls {
            walks: 1,
            read_files: expected_read_files,
            metadata_files: vec![manifest_uri],
        }
    );
}

#[tokio::test]
async fn reads_skill_files_while_resolving_plugin_namespaces() {
    let root = tempdir().expect("tempdir");
    let manifest_path = root.path().join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent")).expect("manifest dir");
    fs::write(&manifest_path, r#"{"name":"parallel"}"#).expect("manifest");
    let skill_path = root.path().join("demo/SKILL.md");
    fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill dir");
    fs::write(
        &skill_path,
        "---\nname: demo\ndescription: demo skill.\n---\n",
    )
    .expect("skill");

    let file_system = RecordingFileSystem::new(
        LOCAL_FS.as_ref(),
        ManifestMetadataBehavior::WaitForSkillRead,
    );
    let root_uri = PathUri::from_host_native_path(root.path()).expect("root URI");
    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        load_environment_skills_from_root(
            &file_system,
            &root_uri,
            /*restriction_product*/ None,
        ),
    )
    .await
    .expect("skill reads should start before namespace resolution finishes");

    assert_eq!(outcome.warnings, Vec::<String>::new());
    assert_eq!(
        outcome.skills,
        vec![EnvironmentSkillMetadata {
            path_to_skills_md: PathUri::from_host_native_path(skill_path).unwrap(),
            name: "parallel:demo".to_string(),
            description: "demo skill.".to_string(),
            short_description: None,
            dependencies: None,
            policy: None,
        }]
    );
}
