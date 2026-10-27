use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigRequirementsToml;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::ExecutorFileSystem;
use codex_exec_server::ExecutorFileSystemFuture;
use codex_exec_server::FileMetadata;
use codex_exec_server::FileSystemReadStream;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::GetMetadataOptions;
use codex_exec_server::LOCAL_FS;
use codex_exec_server::ReadDirectoryEntry;
use codex_exec_server::ReadFileOptions;
use codex_exec_server::RemoveOptions;
use codex_exec_server::WalkOptions;
use codex_exec_server::WalkOutcome;
use codex_exec_server::WriteFileOptions;
use codex_protocol::protocol::SkillScope;
use codex_skills::SkillMetadata;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::PluginIdentity;
use codex_utils_plugins::PluginSkillRoot;
use codex_utils_plugins::SkillDiscoveryMode;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

use super::repo_agents_skill_roots;
use super::resolve_skill_roots_with_home_dir;
use super::roots_from_layer_stack;
use crate::loader::MAX_CONCURRENT_ROOT_SCANS;
use crate::loader::load_and_merge_host_skill_roots;

struct BlockingMetadataFileSystem {
    inner: Arc<dyn ExecutorFileSystem>,
    calls: Arc<BlockingMetadataCalls>,
}

struct BlockingMetadataCalls {
    paths: Mutex<Vec<PathUri>>,
    started: Notify,
    release: Semaphore,
    round_trip: std::time::Duration,
    watch: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    supports_watch: bool,
    watch_ready: Notify,
}

impl Default for BlockingMetadataCalls {
    fn default() -> Self {
        Self {
            paths: Mutex::new(Vec::new()),
            started: Notify::new(),
            release: Semaphore::new(0),
            round_trip: std::time::Duration::ZERO,
            watch: Mutex::default(),
            supports_watch: true,
            watch_ready: Notify::new(),
        }
    }
}

impl ExecutorFileSystem for BlockingMetadataFileSystem {
    fn watch_missing(
        &self,
        missing: Vec<PathUri>,
        present: Option<PathUri>,
    ) -> ExecutorFileSystemFuture<'_, Option<tokio::sync::oneshot::Receiver<()>>> {
        Box::pin(async move {
            if !self.calls.supports_watch {
                return Ok(None);
            }
            if !self.calls.round_trip.is_zero() {
                tokio::time::sleep(self.calls.round_trip).await;
            }
            for path in missing {
                if path.to_abs_path()?.exists() {
                    return Ok(None);
                }
            }
            if let Some(marker) = present
                && !marker.to_abs_path()?.exists()
            {
                return Ok(None);
            }
            let (tx, rx) = tokio::sync::oneshot::channel();
            *self.calls.watch.lock().unwrap() = Some(tx);
            self.calls.watch_ready.notify_one();
            Ok(Some(rx))
        })
    }

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
        options: ReadFileOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, Vec<u8>> {
        self.inner.read_file(path, options, sandbox)
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
        options: WriteFileOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, ()> {
        self.inner.write_file(path, contents, options, sandbox)
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
        options: GetMetadataOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, FileMetadata> {
        Box::pin(async move {
            if !self.calls.round_trip.is_zero() {
                tokio::time::sleep(self.calls.round_trip).await;
            }
            if path
                .to_abs_path()
                .is_ok_and(|path| path.ends_with(Path::new(".agents").join("skills")))
            {
                self.calls
                    .paths
                    .lock()
                    .expect("metadata paths lock")
                    .push(path.clone());
                self.calls.started.notify_one();
                self.calls
                    .release
                    .acquire()
                    .await
                    .expect("metadata release semaphore")
                    .forget();
            }
            self.inner.get_metadata(path, options, sandbox).await
        })
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

fn absolute(path: impl Into<std::path::PathBuf>) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(path.into()).expect("absolute path")
}

fn empty_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

fn stack(layers: Vec<ConfigLayerEntry>) -> ConfigLayerStack {
    ConfigLayerStack::new(
        layers,
        Default::default(),
        ConfigRequirementsToml::default(),
    )
    .expect("valid config stack")
}

fn user_layer(codex_home: &AbsolutePathBuf) -> ConfigLayerEntry {
    ConfigLayerEntry::new(
        ConfigLayerSource::User {
            file: codex_home.join("config.toml"),
            profile: None,
        },
        empty_config(),
    )
}

fn project_layer(dot_codex_folder: &AbsolutePathBuf) -> ConfigLayerEntry {
    ConfigLayerEntry::new(
        ConfigLayerSource::Project {
            dot_codex_folder: dot_codex_folder.clone(),
        },
        empty_config(),
    )
}

fn write_skill(root: &AbsolutePathBuf, directory: &str, name: &str) -> AbsolutePathBuf {
    let skill_dir = root.join(directory);
    fs::create_dir_all(&skill_dir).expect("create skill directory");
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(
        &skill_path,
        format!("---\nname: {name}\ndescription: {name} description\n---\n"),
    )
    .expect("write skill");
    AbsolutePathBuf::from_absolute_path(
        dunce::canonicalize(skill_path).expect("canonical skill path"),
    )
    .expect("absolute skill path")
}

fn expected_skill(path: AbsolutePathBuf, name: &str, scope: SkillScope) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: format!("{name} description"),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: PathUri::from_abs_path(&path),
        scope,
        plugin_id: None,
        remote_plugin_id: None,
    }
}

#[test]
fn layer_roots_preserve_scope_precedence_and_disabled_projects() {
    let temp_dir = TempDir::new().expect("temp dir");
    let system_folder = absolute(temp_dir.path().join("etc/codex"));
    let home_folder = absolute(temp_dir.path().join("home"));
    let user_folder = home_folder.join("codex");
    let project_folder = absolute(temp_dir.path().join("repo/.codex"));
    let nested_project_folder = absolute(temp_dir.path().join("repo/nested/.codex"));
    let config_stack = stack(vec![
        ConfigLayerEntry::new(
            ConfigLayerSource::System {
                file: system_folder.join("config.toml"),
            },
            empty_config(),
        ),
        user_layer(&user_folder),
        ConfigLayerEntry::new_disabled(
            ConfigLayerSource::Project {
                dot_codex_folder: project_folder.clone(),
            },
            empty_config(),
            "untrusted project",
        ),
        project_layer(&nested_project_folder),
    ]);

    let roots = roots_from_layer_stack(
        &config_stack,
        Some(&home_folder),
        Some(Arc::clone(&LOCAL_FS)),
    )
    .into_iter()
    .map(|root| (root.scope, root.path))
    .collect::<Vec<_>>();

    assert_eq!(
        roots,
        vec![
            (SkillScope::Repo, nested_project_folder.join("skills")),
            (SkillScope::Repo, project_folder.join("skills")),
            (SkillScope::User, user_folder.join("skills")),
            (SkillScope::User, home_folder.join(".agents/skills")),
            (SkillScope::System, user_folder.join("skills/.system")),
            (SkillScope::Admin, system_folder.join("skills")),
        ]
    );
}

#[tokio::test]
async fn plugin_roots_preserve_plugin_resolution_metadata() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute(temp_dir.path().join("workspace"));
    let plugin_root = absolute(temp_dir.path().join("plugins/example"));
    let skills_root = plugin_root.join("skills");
    let plugin_identity = PluginIdentity {
        plugin_id: "example@test".to_string(),
        remote_plugin_id: Some("plugins~Plugin_example".to_string()),
    };
    let plugin_namespace = "example".to_string();

    let roots = resolve_skill_roots_with_home_dir(
        /*repository_file_system*/ None,
        &stack(Vec::new()),
        &cwd,
        /*home_dir*/ None,
        vec![PluginSkillRoot {
            path: skills_root.clone(),
            plugin_identity: plugin_identity.clone(),
            plugin_namespace: plugin_namespace.clone(),
            plugin_root: plugin_root.clone(),
            discovery_mode: SkillDiscoveryMode::DirectChildren,
        }],
        Vec::new(),
        /*empty_remote_roots*/ None,
    )
    .await;

    assert_eq!(roots.len(), 1);
    let root = &roots[0];
    assert_eq!(
        (
            root.path.clone(),
            root.scope,
            root.plugin_identity().cloned(),
            root.plugin_namespace().map(str::to_string),
            root.plugin_root().cloned(),
            root.discovery_mode(),
        ),
        (
            skills_root,
            SkillScope::User,
            Some(plugin_identity),
            Some(plugin_namespace),
            Some(plugin_root),
            SkillDiscoveryMode::DirectChildren,
        )
    );
    assert!(Arc::ptr_eq(&root.file_system, &LOCAL_FS));
}

#[tokio::test]
async fn unique_extra_root_loads_as_recursive_user_root() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = absolute(temp_dir.path().join("workspace"));
    let extra_root = absolute(temp_dir.path().join("runtime-skills"));
    let skill_path = write_skill(&extra_root, "runtime", "runtime-skill");

    let roots = resolve_skill_roots_with_home_dir(
        /*repository_file_system*/ None,
        &stack(Vec::new()),
        &cwd,
        /*home_dir*/ None,
        Vec::new(),
        vec![extra_root.clone()],
        /*empty_remote_roots*/ None,
    )
    .await;

    assert_eq!(roots.len(), 1);
    let root = &roots[0];
    assert_eq!(
        (
            root.path.clone(),
            root.scope,
            root.plugin_identity().cloned(),
            root.plugin_namespace().map(str::to_string),
            root.plugin_root().cloned(),
            root.discovery_mode(),
        ),
        (
            extra_root,
            SkillScope::User,
            None,
            None,
            None,
            SkillDiscoveryMode::Recursive,
        )
    );
    assert!(Arc::ptr_eq(&root.file_system, &LOCAL_FS));

    let outcome = load_and_merge_host_skill_roots(
        roots,
        &Semaphore::new(MAX_CONCURRENT_ROOT_SCANS),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert!(outcome.errors.is_empty());
    assert_eq!(
        outcome.skills,
        vec![expected_skill(
            skill_path,
            "runtime-skill",
            SkillScope::User,
        )]
    );
}

#[tokio::test]
async fn repo_ancestry_without_project_marker_does_not_walk_parents() {
    let temp_dir = TempDir::new().expect("temp dir");
    let outer = absolute(temp_dir.path().join("outer"));
    let cwd = outer.join("nested/inner");
    fs::create_dir_all(outer.join(".agents/skills")).expect("create outer skills");
    fs::create_dir_all(cwd.join(".agents/skills")).expect("create cwd skills");

    let roots = repo_agents_skill_roots(
        Some(Arc::clone(&LOCAL_FS)),
        &stack(Vec::new()),
        &cwd,
        /*empty_remote_roots*/ None,
    )
    .await
    .into_iter()
    .map(|root| root.path)
    .collect::<Vec<_>>();

    assert_eq!(roots, vec![cwd.join(".agents/skills")]);
}

/// Repeat the two remote metadata rounds with 74 ms per RPC and no repository skills.
#[tokio::test]
#[ignore = "manual remote filesystem latency comparison"]
async fn time_empty_remote_repo_skill_discovery() {
    let temp = TempDir::new().unwrap();
    let root = absolute(temp.path());
    let cwd = root.join("workspace/nested");
    fs::create_dir_all(&cwd).unwrap();
    fs::write(root.join(".git"), "gitdir: repo\n").unwrap();
    let calls = Arc::new(BlockingMetadataCalls {
        round_trip: std::time::Duration::from_millis(74),
        ..Default::default()
    });
    calls.release.add_permits(100);
    let remote: Arc<dyn ExecutorFileSystem> = Arc::new(BlockingMetadataFileSystem {
        inner: Arc::clone(&LOCAL_FS),
        calls: Arc::clone(&calls),
    });
    let config = stack(Vec::new());
    let cache = Mutex::new(super::EmptyRemoteRootsCache::default());
    for (label, cache) in [("baseline", None), ("warm", Some(&cache))] {
        // Prewarm before measuring admission, as session initialization does.
        let setup = std::time::Instant::now();
        assert!(
            repo_agents_skill_roots(Some(remote.clone()), &config, &cwd, cache)
                .await
                .is_empty()
        );
        eprintln!("{label} first scan: {:?}", setup.elapsed());
        if cache.is_some() {
            let registration = std::time::Instant::now();
            calls.watch_ready.notified().await;
            eprintln!("background registration: {:?}", registration.elapsed());
        }
        let mut times = Vec::new();
        for _ in 0..20 {
            let start = std::time::Instant::now();
            assert!(
                repo_agents_skill_roots(Some(remote.clone()), &config, &cwd, cache)
                    .await
                    .is_empty()
            );
            times.push(start.elapsed());
        }
        times.sort_unstable();
        eprintln!("{label}, n=20, p50={:?}, p95={:?}", times[9], times[18]);
    }
}

#[tokio::test(start_paused = true)]
async fn empty_remote_repo_skills_are_reused_until_watch_invalidation() {
    let temp = TempDir::new().unwrap();
    let cwd = absolute(temp.path());
    fs::write(cwd.join(".git"), "gitdir: repo\n").unwrap();
    let calls = Arc::new(BlockingMetadataCalls::default());
    calls.release.add_permits(10);
    let remote: Arc<dyn ExecutorFileSystem> = Arc::new(BlockingMetadataFileSystem {
        inner: Arc::clone(&LOCAL_FS),
        calls: Arc::clone(&calls),
    });
    let cache = Mutex::new(super::EmptyRemoteRootsCache::default());
    let config = stack(Vec::new());
    let discover =
        || repo_agents_skill_roots(Some(Arc::clone(&remote)), &config, &cwd, Some(&cache));
    assert!(discover().await.is_empty());
    calls.watch_ready.notified().await;
    // No clock deadline or polling during an idle connection.
    tokio::time::advance(std::time::Duration::from_secs(3600)).await;
    assert!(discover().await.is_empty());
    assert_eq!(calls.paths.lock().unwrap().len(), 1);

    let skills = cwd.join(".agents/skills");
    fs::create_dir_all(&skills).unwrap();
    calls.watch.lock().unwrap().take();
    assert_eq!(discover().await[0].path, skills);
    fs::remove_dir(&skills).unwrap();
    assert!(discover().await.is_empty());
    fs::create_dir(&skills).unwrap();
    cache.lock().unwrap().clear();
    assert_eq!(discover().await[0].path, skills);
}

#[tokio::test(start_paused = true)]
async fn newly_created_roots_are_visible_before_watch_ack_or_without_watch_support() {
    for supports_watch in [true, false] {
        let temp = TempDir::new().unwrap();
        let cwd = absolute(temp.path());
        fs::write(cwd.join(".git"), "").unwrap();
        let calls = Arc::new(BlockingMetadataCalls {
            supports_watch,
            round_trip: std::time::Duration::from_millis(74),
            ..Default::default()
        });
        calls.release.add_permits(2);
        let remote: Arc<dyn ExecutorFileSystem> = Arc::new(BlockingMetadataFileSystem {
            inner: Arc::clone(&LOCAL_FS),
            calls,
        });
        let cache = Mutex::new(super::EmptyRemoteRootsCache::default());
        let config = stack(Vec::new());
        assert!(
            repo_agents_skill_roots(Some(remote.clone()), &config, &cwd, Some(&cache))
                .await
                .is_empty()
        );
        let skills = cwd.join(".agents/skills");
        fs::create_dir_all(&skills).unwrap();
        assert_eq!(
            repo_agents_skill_roots(Some(remote), &config, &cwd, Some(&cache)).await[0].path,
            skills
        );
    }
}

#[tokio::test]
async fn repo_ancestry_stops_at_project_root_and_preserves_root_to_cwd_order() {
    let temp_dir = TempDir::new().expect("temp dir");
    let outer = absolute(temp_dir.path().join("outer"));
    let repository = outer.join("repo");
    let nested = repository.join("nested/inner");
    fs::create_dir_all(&nested).expect("create nested cwd");
    fs::write(repository.join(".git"), "gitdir: fake\n").expect("write git marker");
    fs::create_dir_all(outer.join(".agents/skills")).expect("create outer skills");
    fs::create_dir_all(repository.join(".agents/skills")).expect("create repo skills");
    fs::create_dir_all(repository.join("nested/.agents/skills")).expect("create nested skills");
    let config_stack = stack(Vec::new());

    let roots = repo_agents_skill_roots(
        Some(Arc::clone(&LOCAL_FS)),
        &config_stack,
        &nested,
        /*empty_remote_roots*/ None,
    )
    .await
    .into_iter()
    .map(|root| root.path)
    .collect::<Vec<_>>();

    assert_eq!(
        roots,
        vec![
            repository.join(".agents/skills"),
            repository.join("nested/.agents/skills"),
        ]
    );
}

#[tokio::test]
async fn resolved_project_layer_loads_skill_without_git_marker() {
    let temp_dir = TempDir::new().expect("temp dir");
    let workspace = absolute(temp_dir.path().join("workspace"));
    let dot_codex = workspace.join(".codex");
    let skill_root = dot_codex.join("skills");
    fs::create_dir_all(&workspace).expect("create workspace");
    let skill_path = write_skill(&skill_root, "local", "local-skill");
    let config_stack = stack(vec![project_layer(&dot_codex)]);

    let roots = resolve_skill_roots_with_home_dir(
        Some(Arc::clone(&LOCAL_FS)),
        &config_stack,
        &workspace,
        /*home_dir*/ None,
        Vec::new(),
        Vec::new(),
        /*empty_remote_roots*/ None,
    )
    .await;
    let outcome = load_and_merge_host_skill_roots(
        roots,
        &Semaphore::new(MAX_CONCURRENT_ROOT_SCANS),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert!(outcome.errors.is_empty());
    assert_eq!(
        outcome.skills,
        vec![expected_skill(skill_path, "local-skill", SkillScope::Repo)]
    );
}

#[tokio::test]
async fn resolved_project_layer_loads_skill_when_cwd_is_file() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repository = absolute(temp_dir.path().join("repo"));
    let dot_codex = repository.join(".codex");
    let skill_root = dot_codex.join("skills");
    fs::create_dir_all(&repository).expect("create repository");
    fs::write(repository.join(".git"), "gitdir: fake\n").expect("write git marker");
    let cwd = repository.join("some-file.txt");
    fs::write(&cwd, "contents").expect("write cwd file");
    let skill_path = write_skill(&skill_root, "repo", "repo-skill");
    let config_stack = stack(vec![project_layer(&dot_codex)]);

    let roots = resolve_skill_roots_with_home_dir(
        Some(Arc::clone(&LOCAL_FS)),
        &config_stack,
        &cwd,
        /*home_dir*/ None,
        Vec::new(),
        Vec::new(),
        /*empty_remote_roots*/ None,
    )
    .await;
    let outcome = load_and_merge_host_skill_roots(
        roots,
        &Semaphore::new(MAX_CONCURRENT_ROOT_SCANS),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;

    assert!(outcome.errors.is_empty());
    assert_eq!(
        outcome.skills,
        vec![expected_skill(skill_path, "repo-skill", SkillScope::Repo)]
    );
}

#[tokio::test]
async fn repo_ancestry_limits_concurrent_probes_and_preserves_order() {
    const CONCURRENCY_LIMIT: usize = 256;

    let temp_dir = TempDir::new().expect("temp dir");
    let repository = absolute(temp_dir.path().join("repo"));
    fs::create_dir_all(&repository).expect("create repository");
    fs::write(repository.join(".git"), "gitdir: fake\n").expect("write git marker");

    let mut directories = vec![repository.clone()];
    let mut cwd = repository;
    for _ in 0..CONCURRENCY_LIMIT {
        cwd = cwd.join("d");
        directories.push(cwd.clone());
    }
    fs::create_dir_all(&cwd).expect("create nested cwd");

    let expected_roots = [0, CONCURRENCY_LIMIT / 2, CONCURRENCY_LIMIT].map(|index| {
        let path = directories[index].join(".agents/skills");
        fs::create_dir_all(&path).expect("create repo skill root");
        path
    });
    let expected_probes = directories
        .iter()
        .map(|directory| PathUri::from_abs_path(&directory.join(".agents/skills")))
        .collect::<Vec<_>>();
    let calls = Arc::new(BlockingMetadataCalls::default());
    let file_system: Arc<dyn ExecutorFileSystem> = Arc::new(BlockingMetadataFileSystem {
        inner: Arc::clone(&LOCAL_FS),
        calls: Arc::clone(&calls),
    });

    let assertions = async {
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), async {
            loop {
                let started = calls.started.notified();
                if calls.paths.lock().expect("metadata paths lock").len() >= CONCURRENCY_LIMIT {
                    break;
                }
                started.await;
            }
        })
        .await
        .expect("initial repo skill root window should start");
        assert_eq!(
            calls.paths.lock().expect("metadata paths lock").as_slice(),
            &expected_probes[..CONCURRENCY_LIMIT]
        );

        calls.release.add_permits(/*n*/ 1);
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), async {
            loop {
                let started = calls.started.notified();
                if calls.paths.lock().expect("metadata paths lock").len() > CONCURRENCY_LIMIT {
                    break;
                }
                started.await;
            }
        })
        .await
        .expect("next repo skill root probe should start");
        assert_eq!(
            calls.paths.lock().expect("metadata paths lock").as_slice(),
            expected_probes.as_slice()
        );

        calls.release.add_permits(expected_probes.len());
    };
    let config_stack = stack(Vec::new());
    let (roots, ()) = tokio::join!(
        repo_agents_skill_roots(
            Some(file_system),
            &config_stack,
            &cwd,
            /*empty_remote_roots*/ None
        ),
        assertions,
    );

    assert_eq!(
        roots.into_iter().map(|root| root.path).collect::<Vec<_>>(),
        expected_roots
    );
}

#[tokio::test]
async fn resolved_config_and_repo_roots_preserve_order_and_dedupe_paths_not_names() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_folder = absolute(temp_dir.path().join("home"));
    let codex_home = home_folder.join("codex");
    let system_folder = absolute(temp_dir.path().join("etc/codex"));
    let repository = absolute(temp_dir.path().join("repo"));
    let cwd = repository.join("nested/inner");
    fs::create_dir_all(&cwd).expect("create cwd");
    fs::write(repository.join(".git"), "gitdir: fake\n").expect("write git marker");

    let project_dot_codex = repository.join(".codex");
    let nested_project_dot_codex = repository.join("nested/.codex");
    let user_skills = codex_home.join("skills");
    let root_project_skill = write_skill(
        &project_dot_codex.join("skills"),
        "root-duplicate",
        "duplicate-skill",
    );
    let nested_project_skill = write_skill(
        &nested_project_dot_codex.join("skills"),
        "nested-duplicate",
        "duplicate-skill",
    );
    let user_skill = write_skill(&user_skills, "user-duplicate", "duplicate-skill");
    let home_skill = write_skill(&home_folder.join(".agents/skills"), "home", "home-skill");
    let system_skill = write_skill(&codex_home.join("skills/.system"), "system", "system-skill");
    let admin_skill = write_skill(&system_folder.join("skills"), "admin", "admin-skill");
    let repo_agent_skill = write_skill(
        &repository.join(".agents/skills"),
        "repo-agent",
        "repo-agent-skill",
    );
    let nested_agent_skill = write_skill(
        &repository.join("nested/.agents/skills"),
        "nested-agent",
        "nested-agent-skill",
    );
    let config_stack = stack(vec![
        ConfigLayerEntry::new(
            ConfigLayerSource::System {
                file: system_folder.join("config.toml"),
            },
            empty_config(),
        ),
        user_layer(&codex_home),
        project_layer(&project_dot_codex),
        project_layer(&nested_project_dot_codex),
    ]);

    let roots = resolve_skill_roots_with_home_dir(
        Some(Arc::clone(&LOCAL_FS)),
        &config_stack,
        &cwd,
        Some(&home_folder),
        Vec::new(),
        vec![user_skills],
        /*empty_remote_roots*/ None,
    )
    .await;
    assert_eq!(roots.len(), 8);
    let outcome = load_and_merge_host_skill_roots(
        roots,
        &Semaphore::new(MAX_CONCURRENT_ROOT_SCANS),
        /*restriction_product*/ None,
        /*plugin_skill_snapshots*/ None,
    )
    .await;
    assert!(outcome.errors.is_empty());
    assert_eq!(
        outcome.skills,
        vec![
            expected_skill(root_project_skill, "duplicate-skill", SkillScope::Repo),
            expected_skill(nested_project_skill, "duplicate-skill", SkillScope::Repo),
            expected_skill(nested_agent_skill, "nested-agent-skill", SkillScope::Repo),
            expected_skill(repo_agent_skill, "repo-agent-skill", SkillScope::Repo),
            expected_skill(user_skill, "duplicate-skill", SkillScope::User),
            expected_skill(home_skill, "home-skill", SkillScope::User),
            expected_skill(system_skill, "system-skill", SkillScope::System),
            expected_skill(admin_skill, "admin-skill", SkillScope::Admin),
        ]
    );
}
