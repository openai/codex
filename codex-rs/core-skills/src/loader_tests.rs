use super::*;
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
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::PathExt;
use codex_utils_path_uri::PathUri;
use dunce::canonicalize as canonicalize_path;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tempfile::TempDir;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

const REPO_ROOT_CONFIG_DIR_NAME: &str = ".codex";
const SKILLS_DIR_NAME: &str = "skills";

struct BlockingRepoSkillRootFileSystem {
    inner: Arc<dyn ExecutorFileSystem>,
    blocked_walk_root: Option<PathUri>,
    blocked_walk_gate: Semaphore,
    walks_started: AtomicUsize,
    walk_started: Notify,
}

impl ExecutorFileSystem for BlockingRepoSkillRootFileSystem {
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

    fn walk<'a>(
        &'a self,
        path: &'a PathUri,
        options: WalkOptions,
        sandbox: Option<&'a FileSystemSandboxContext>,
    ) -> ExecutorFileSystemFuture<'a, WalkOutcome> {
        self.walks_started.fetch_add(/*val*/ 1, Ordering::AcqRel);
        self.walk_started.notify_waiters();
        if self.blocked_walk_root.as_ref() != Some(path) {
            return self.inner.walk(path, options, sandbox);
        }
        Box::pin(async move {
            self.blocked_walk_gate
                .acquire()
                .await
                .expect("blocked walk gate should remain open")
                .forget();
            self.inner.walk(path, options, sandbox).await
        })
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

async fn load_skills_for_test<I>(roots: I) -> SkillLoadOutcome
where
    I: IntoIterator<Item = SkillRoot> + Send,
    I::IntoIter: Send,
{
    super::load_skills_from_roots(
        roots,
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await
}

fn local_skill_root(path: &Path, scope: SkillScope) -> SkillRoot {
    SkillRoot {
        path: path.abs(),
        scope,
        file_system: Arc::clone(&LOCAL_FS),
        plugin_identity: None,
        plugin_namespace: None,
        plugin_root: None,
        discovery_mode: SkillDiscoveryMode::Recursive,
    }
}

async fn load_user_skills_for_test(codex_home: &TempDir) -> SkillLoadOutcome {
    load_skills_for_test([local_skill_root(
        &codex_home.path().join(SKILLS_DIR_NAME),
        SkillScope::User,
    )])
    .await
}

fn normalized(path: &Path) -> AbsolutePathBuf {
    canonicalize_path(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .abs()
}
fn write_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) -> PathBuf {
    write_skill_at(&codex_home.path().join("skills"), dir, name, description)
}

fn write_system_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) -> PathBuf {
    write_skill_at(
        &codex_home.path().join("skills/.system"),
        dir,
        name,
        description,
    )
}

fn write_skill_at(root: &Path, dir: &str, name: &str, description: &str) -> PathBuf {
    let skill_dir = root.join(dir);
    fs::create_dir_all(&skill_dir).unwrap();
    let indented_description = description.replace('\n', "\n  ");
    let content =
        format!("---\nname: {name}\ndescription: |-\n  {indented_description}\n---\n\n# Body\n");
    let path = skill_dir.join(SKILLS_FILENAME);
    fs::write(&path, content).unwrap();
    path
}

fn write_raw_skill_at(root: &Path, dir: &str, frontmatter: &str) -> PathBuf {
    let skill_dir = root.join(dir);
    fs::create_dir_all(&skill_dir).unwrap();
    let path = skill_dir.join(SKILLS_FILENAME);
    let content = format!("---\n{frontmatter}\n---\n\n# Body\n");
    fs::write(&path, content).unwrap();
    path
}

fn write_skill_metadata_at(skill_dir: &Path, contents: &str) -> PathBuf {
    let path = skill_dir
        .join(SKILLS_METADATA_DIR)
        .join(SKILLS_METADATA_FILENAME);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    path
}

fn write_skill_interface_at(skill_dir: &Path, contents: &str) -> PathBuf {
    write_skill_metadata_at(skill_dir, contents)
}

fn write_plugin_manifest(plugin_root: &Path, contents: &str) {
    let manifest_path = plugin_root.join(".codex-plugin/plugin.json");
    fs::create_dir_all(manifest_path.parent().expect("manifest parent")).unwrap();
    fs::write(manifest_path, contents).unwrap();
}

async fn load_user_skills_root(root: &Path) -> SkillLoadOutcome {
    load_skills_from_roots(
        [SkillRoot {
            path: root.abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: None,
            plugin_namespace: None,
            plugin_root: None,
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await
}

fn expected_user_skill(path: &Path, name: &str, description: &str) -> SkillMetadata {
    SkillMetadata {
        name: name.to_string(),
        description: description.to_string(),
        short_description: None,
        interface: None,
        dependencies: None,
        policy: None,
        path_to_skills_md: normalized(path),
        scope: SkillScope::User,
        plugin_id: None,
        remote_plugin_id: None,
    }
}

#[tokio::test]
async fn loads_skill_dependencies_metadata_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "dep-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
{
  "dependencies": {
    "tools": [
      {
        "type": "mcp",
        "value": "github",
        "description": "GitHub MCP server",
        "transport": "streamable_http",
        "url": "https://example.com/mcp"
      },
      {
        "type": "cli",
        "value": "gh",
        "description": "GitHub CLI"
      },
      {
        "type": "mcp",
        "value": "local-gh",
        "description": "Local GH MCP server",
        "transport": "stdio",
        "command": "gh-mcp"
      }
    ]
  }
}
"#,
    );

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "dep-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: None,
            dependencies: Some(SkillDependencies {
                tools: vec![
                    SkillToolDependency {
                        r#type: "mcp".to_string(),
                        value: "github".to_string(),
                        description: Some("GitHub MCP server".to_string()),
                        transport: Some("streamable_http".to_string()),
                        command: None,
                        url: Some("https://example.com/mcp".to_string()),
                    },
                    SkillToolDependency {
                        r#type: "cli".to_string(),
                        value: "gh".to_string(),
                        description: Some("GitHub CLI".to_string()),
                        transport: None,
                        command: None,
                        url: None,
                    },
                    SkillToolDependency {
                        r#type: "mcp".to_string(),
                        value: "local-gh".to_string(),
                        description: Some("Local GH MCP server".to_string()),
                        transport: Some("stdio".to_string()),
                        command: Some("gh-mcp".to_string()),
                        url: None,
                    },
                ],
            }),
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_skill_interface_metadata_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "ui-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");
    let normalized_skill_dir = normalized(skill_dir);

    write_skill_interface_at(
        skill_dir,
        r##"
interface:
  display_name: "UI Skill"
  short_description: "  short    desc   "
  icon_small: "./assets/small-400px.png"
  icon_large: "./assets/large-logo.svg"
  brand_color: "#3B82F6"
  default_prompt: "  default   prompt   "
"##,
    );

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    let user_skills: Vec<SkillMetadata> = outcome
        .skills
        .into_iter()
        .filter(|skill| skill.scope == SkillScope::User)
        .collect();
    assert_eq!(
        user_skills,
        vec![SkillMetadata {
            name: "ui-skill".to_string(),
            description: "from json".to_string(),
            short_description: None,
            interface: Some(SkillInterface {
                display_name: Some("UI Skill".to_string()),
                short_description: Some("short desc".to_string()),
                icon_small: Some(normalized_skill_dir.join("assets/small-400px.png")),
                icon_large: Some(normalized_skill_dir.join("assets/large-logo.svg")),
                brand_color: Some("#3B82F6".to_string()),
                default_prompt: Some("default prompt".to_string()),
            }),
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(skill_path.as_path()),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_skill_policy_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "policy-skill", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
policy:
  allow_implicit_invocation: false
"#,
    );

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].policy,
        Some(SkillPolicy {
            allow_implicit_invocation: Some(false),
            products: vec![],
        })
    );
    assert!(outcome.allowed_skills_for_implicit_invocation().is_empty());
}

#[tokio::test]
async fn empty_skill_policy_defaults_to_allow_implicit_invocation() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "policy-empty", "from json");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
policy: {}
"#,
    );

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].policy,
        Some(SkillPolicy {
            allow_implicit_invocation: None,
            products: vec![],
        })
    );
    assert_eq!(
        outcome.allowed_skills_for_implicit_invocation(),
        outcome.skills
    );
}

#[tokio::test]
async fn loads_skill_policy_products_from_yaml() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "policy-products", "from yaml");
    let skill_dir = skill_path.parent().expect("skill dir");

    write_skill_metadata_at(
        skill_dir,
        r#"
policy:
  products:
    - codex
    - CHATGPT
    - atlas
"#,
    );

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        outcome.skills[0].policy,
        Some(SkillPolicy {
            allow_implicit_invocation: None,
            products: vec![Product::Codex, Product::Chatgpt, Product::Atlas],
        })
    );
}

#[tokio::test]
async fn loads_plugin_skill_interface_icons_from_shared_plugin_assets() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("plugins/twilio-developer-kit");
    let skill_path = write_skill_at(
        &plugin_root.join("skills"),
        "twilio-send-message",
        "send-message",
        "send messages",
    );
    let skill_dir = skill_path.parent().expect("skill dir");
    fs::create_dir_all(plugin_root.join("assets")).unwrap();
    fs::write(plugin_root.join("assets/logo.svg"), "<svg/>").unwrap();
    write_skill_interface_at(
        skill_dir,
        r##"
interface:
  icon_small: "../../assets/logo.svg"
  icon_large: "../../assets/logo.svg"
"##,
    );

    let plugin_root_abs = plugin_root.abs();
    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: plugin_root.join("skills").abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: Some(PluginIdentity {
                plugin_id: "twilio-developer-kit@test".to_string(),
                remote_plugin_id: None,
            }),
            plugin_namespace: None,
            plugin_root: Some(plugin_root_abs.clone()),
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    let expected_icon_path = normalized(&plugin_root.join("assets/logo.svg"));
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "send-message".to_string(),
            description: "send messages".to_string(),
            short_description: None,
            interface: Some(SkillInterface {
                display_name: None,
                short_description: None,
                icon_small: Some(expected_icon_path.clone()),
                icon_large: Some(expected_icon_path),
                brand_color: None,
                default_prompt: None,
            }),
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: Some("twilio-developer-kit@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[tokio::test]
#[cfg(unix)]
async fn loads_skills_via_symlinked_subdir_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    let shared_skill_path = write_skill_at(shared.path(), "demo", "linked-skill", "from link");

    fs::create_dir_all(codex_home.path().join("skills")).unwrap();
    symlink_dir(shared.path(), &codex_home.path().join("skills/shared"));

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "linked-skill".to_string(),
            description: "from link".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&shared_skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
    let canonical_skill_path = normalized(&shared_skill_path);
    let discovery_path = outcome
        .skill_root_for_path(&canonical_skill_path)
        .expect("symlinked skill should retain its discovery root")
        .join("shared/demo/SKILL.md");
    assert_eq!(
        outcome.skill_discovery_path_for_path(&canonical_skill_path),
        Some(&discovery_path)
    );
    let filtered_outcome =
        crate::filter_skill_load_outcome_for_product(outcome, Some(Product::Codex));
    assert_eq!(
        filtered_outcome.skill_discovery_path_for_path(&canonical_skill_path),
        Some(&discovery_path)
    );
}

// Directory symlinks on Windows can require Developer Mode or administrator privileges.
#[tokio::test]
#[cfg(unix)]
async fn loads_skills_through_visible_alias_to_hidden_directory() {
    let root = tempfile::tempdir().expect("tempdir");
    let hidden_root = root.path().join(".hidden");
    let skill_path = write_skill_at(&hidden_root, "search", "search-skill", "search description");
    symlink_dir(&hidden_root, &root.path().join("visible"));

    let outcome = load_user_skills_root(root.path()).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![expected_user_skill(
            &skill_path,
            "search-skill",
            "search description",
        )]
    );
}

#[tokio::test]
#[cfg(unix)]
async fn ignores_symlinked_skill_file_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    let shared_skill_path = write_skill_at(shared.path(), "demo", "linked-file-skill", "from link");

    let skill_dir = codex_home.path().join("skills/demo");
    fs::create_dir_all(&skill_dir).unwrap();
    symlink_file(&shared_skill_path, &skill_dir.join(SKILLS_FILENAME));

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills, Vec::new());
}

#[tokio::test]
#[cfg(unix)]
async fn does_not_loop_on_symlink_cycle_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");

    // Create a cycle:
    //   $CODEX_HOME/skills/cycle/loop -> $CODEX_HOME/skills/cycle
    let cycle_dir = codex_home.path().join("skills/cycle");
    fs::create_dir_all(&cycle_dir).unwrap();
    symlink_dir(&cycle_dir, &cycle_dir.join("loop"));

    let skill_path = write_skill_at(&cycle_dir, "demo", "cycle-skill", "still loads");

    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "cycle-skill".to_string(),
            description: "still loads".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
#[cfg(unix)]
async fn loads_skills_via_symlinked_subdir_for_admin_scope() {
    let admin_root = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    let shared_skill_path =
        write_skill_at(shared.path(), "demo", "admin-linked-skill", "from link");
    fs::create_dir_all(admin_root.path()).unwrap();
    symlink_dir(shared.path(), &admin_root.path().join("shared"));

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: admin_root.path().abs(),
            scope: SkillScope::Admin,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: None,
            plugin_namespace: None,
            plugin_root: None,
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "admin-linked-skill".to_string(),
            description: "from link".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&shared_skill_path),
            scope: SkillScope::Admin,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
#[cfg(unix)]
async fn loads_skills_via_symlinked_subdir_for_repo_scope() {
    let repo_dir = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    let linked_skill_path = write_skill_at(shared.path(), "demo", "repo-linked-skill", "from link");
    let repo_skills_root = repo_dir
        .path()
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    fs::create_dir_all(&repo_skills_root).unwrap();
    symlink_dir(shared.path(), &repo_skills_root.join("shared"));

    let outcome =
        load_skills_for_test([local_skill_root(&repo_skills_root, SkillScope::Repo)]).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "repo-linked-skill".to_string(),
            description: "from link".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&linked_skill_path),
            scope: SkillScope::Repo,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
    let canonical_skill_path = normalized(&linked_skill_path);
    let discovery_path = outcome
        .skill_root_for_path(&canonical_skill_path)
        .expect("repo skill should retain its discovery root")
        .join("shared/demo/SKILL.md");
    assert_eq!(
        outcome.skill_discovery_path_for_path(&canonical_skill_path),
        Some(&discovery_path)
    );
}

#[tokio::test]
#[cfg(unix)]
async fn system_scope_ignores_symlinked_subdir() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let shared = tempfile::tempdir().expect("tempdir");

    write_skill_at(shared.path(), "demo", "system-linked-skill", "from link");

    let system_root = codex_home.path().join("skills/.system");
    fs::create_dir_all(&system_root).unwrap();
    symlink_dir(shared.path(), &system_root.join("shared"));

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: system_root.abs(),
            scope: SkillScope::System,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: None,
            plugin_namespace: None,
            plugin_root: None,
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 0);
}

#[tokio::test]
async fn respects_max_scan_depth_for_user_scope() {
    let codex_home = tempfile::tempdir().expect("tempdir");

    let within_depth_path = write_skill(
        &codex_home,
        "d0/d1/d2/d3/d4/d5",
        "within-depth-skill",
        "loads",
    );
    let _too_deep_path = write_skill(
        &codex_home,
        "d0/d1/d2/d3/d4/d5/d6",
        "too-deep-skill",
        "should not load",
    );

    let skills_root = codex_home.path().join("skills");
    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: skills_root.abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: None,
            plugin_namespace: None,
            plugin_root: None,
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "within-depth-skill".to_string(),
            description: "loads".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&within_depth_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_valid_skill() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill(&codex_home, "demo", "demo-skill", "does things\ncarefully");
    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "demo-skill".to_string(),
            description: "does things carefully".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn falls_back_to_directory_name_when_skill_name_is_missing() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_raw_skill_at(
        &codex_home.path().join("skills"),
        "directory-derived",
        "description: fallback name",
    );
    let outcome = load_user_skills_for_test(&codex_home).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "directory-derived".to_string(),
            description: "fallback name".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn namespaces_plugin_skills_using_provided_namespace() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("plugins/sample");
    let skill_path = write_raw_skill_at(
        &plugin_root.join("skills"),
        "sample-search",
        "description: search sample data",
    );
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"should-not-be-read"}"#,
    )
    .unwrap();

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: plugin_root.join("skills").abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: Some(PluginIdentity {
                plugin_id: "sample@test".to_string(),
                remote_plugin_id: None,
            }),
            plugin_namespace: Some("sample".to_string()),
            plugin_root: Some(plugin_root.abs()),
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "sample:sample-search".to_string(),
            description: "search sample data".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: Some("sample@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn namespaces_nested_plugin_skills_without_namespacing_plain_siblings() {
    let root = tempfile::tempdir().expect("tempdir");
    let skills_root = root.path().join("skills");
    let plain_skill_path =
        write_skill_at(&skills_root, "plain", "plain-skill", "plain description");
    let plugin_root = skills_root.join("nested-plugin");
    write_plugin_manifest(&plugin_root, r#"{"name":"nested"}"#);
    let plugin_skill_path = write_skill_at(
        &plugin_root.join("skills"),
        "search",
        "plugin-skill",
        "plugin description",
    );

    let outcome = load_user_skills_root(&skills_root).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![
            expected_user_skill(
                &plugin_skill_path,
                "nested:plugin-skill",
                "plugin description"
            ),
            expected_user_skill(&plain_skill_path, "plain-skill", "plain description"),
        ]
    );
}

#[tokio::test]
async fn inherits_plugin_namespace_from_above_scanned_skills_root() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("plugin");
    write_plugin_manifest(&plugin_root, r#"{"name":"outer"}"#);
    let skills_root = plugin_root.join("skills");
    let skill_path = write_skill_at(&skills_root, "search", "search-skill", "search description");

    let outcome = load_user_skills_root(&skills_root).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![expected_user_skill(
            &skill_path,
            "outer:search-skill",
            "search description",
        )]
    );
}

#[tokio::test]
async fn nearest_valid_nested_plugin_namespace_overrides_outer_namespace() {
    let root = tempfile::tempdir().expect("tempdir");
    let outer_plugin_root = root.path().join("outer-plugin");
    write_plugin_manifest(&outer_plugin_root, r#"{"name":"outer"}"#);
    let skills_root = outer_plugin_root.join("skills");
    let nested_plugin_root = skills_root.join("nested-plugin");
    write_plugin_manifest(&nested_plugin_root, r#"{"name":"nested"}"#);
    let skill_path = write_skill_at(
        &nested_plugin_root.join("skills"),
        "search",
        "search-skill",
        "search description",
    );

    let outcome = load_user_skills_root(&skills_root).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![expected_user_skill(
            &skill_path,
            "nested:search-skill",
            "search description",
        )]
    );
}

#[tokio::test]
async fn invalid_nested_plugin_manifest_falls_back_to_outer_namespace() {
    let root = tempfile::tempdir().expect("tempdir");
    let outer_plugin_root = root.path().join("outer-plugin");
    write_plugin_manifest(&outer_plugin_root, r#"{"name":"outer"}"#);
    let skills_root = outer_plugin_root.join("skills");
    let nested_plugin_root = skills_root.join("nested-plugin");
    write_plugin_manifest(&nested_plugin_root, "not json");
    let skill_path = write_skill_at(
        &nested_plugin_root.join("skills"),
        "search",
        "search-skill",
        "search description",
    );

    let outcome = load_user_skills_root(&skills_root).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![expected_user_skill(
            &skill_path,
            "outer:search-skill",
            "search description",
        )]
    );
}

// Directory symlinks on Windows can require Developer Mode or administrator privileges.
#[cfg(unix)]
#[tokio::test]
async fn does_not_inherit_namespace_for_skills_in_symlinked_plain_dir() {
    // outer-plugin/
    // ├── .codex-plugin/plugin.json
    // └── skills/linked-plain -> plain-root/
    // plain-root/
    // └── search/SKILL.md
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("outer-plugin");
    write_plugin_manifest(&plugin_root, r#"{"name":"outer"}"#);
    let skills_root = plugin_root.join("skills");
    let plain_root = tempfile::tempdir().expect("tempdir");
    let skill_path = write_skill_at(
        plain_root.path(),
        "search",
        "plain-skill",
        "plain description",
    );
    fs::create_dir_all(&skills_root).unwrap();
    symlink_dir(plain_root.path(), &skills_root.join("linked-plain"));

    let outcome = load_user_skills_root(&skills_root).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![expected_user_skill(
            &skill_path,
            "plain-skill",
            "plain description",
        )]
    );
}

// Directory symlinks on Windows can require Developer Mode or administrator privileges.
#[cfg(unix)]
#[tokio::test]
async fn keeps_inherited_namespace_when_symlink_target_is_scan_root_ancestor() {
    // temp-root/
    // └── a/b/c/d/e/f/outer-plugin/
    //     ├── .codex-plugin/plugin.json
    //     └── skills/
    //         ├── root/SKILL.md
    //         └── link -> temp-root/
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("a/b/c/d/e/f/outer-plugin");
    write_plugin_manifest(&plugin_root, r#"{"name":"outer"}"#);
    let skills_root = plugin_root.join("skills");
    let skill_path = write_skill_at(&skills_root, "root", "root-skill", "root description");
    symlink_dir(root.path(), &skills_root.join("link"));

    let outcome = load_user_skills_root(&skills_root).await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![expected_user_skill(
            &skill_path,
            "outer:root-skill",
            "root description",
        )]
    );
}

#[tokio::test]
async fn plugin_skill_name_length_limit_allows_max_qualified_name() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_name = "p".repeat(MAX_NAME_LEN);
    let skill_name = "s".repeat(MAX_NAME_LEN);
    let plugin_root = root.path().join("plugins").join(&plugin_name);
    let frontmatter = format!("name: {skill_name}\ndescription: search sample data");
    let skill_path = write_raw_skill_at(&plugin_root.join("skills"), "sample-search", &frontmatter);
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{plugin_name}"}}"#),
    )
    .unwrap();

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: plugin_root.join("skills").abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: Some(PluginIdentity {
                plugin_id: "sample@test".to_string(),
                remote_plugin_id: None,
            }),
            plugin_namespace: Some(plugin_name.clone()),
            plugin_root: Some(plugin_root.abs()),
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: format!("{plugin_name}:{skill_name}"),
            description: "search sample data".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: Some("sample@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn plugin_skill_name_length_limit_rejects_overlong_qualified_name() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_name = "p".repeat(MAX_NAME_LEN + 1);
    let skill_name = "s".repeat(MAX_NAME_LEN);
    let plugin_root = root.path().join("plugins").join(&plugin_name);
    let frontmatter = format!("name: {skill_name}\ndescription: search sample data");
    write_raw_skill_at(&plugin_root.join("skills"), "sample-search", &frontmatter);
    fs::create_dir_all(plugin_root.join(".codex-plugin")).unwrap();
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"{plugin_name}"}}"#),
    )
    .unwrap();

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: plugin_root.join("skills").abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: Some(PluginIdentity {
                plugin_id: "sample@test".to_string(),
                remote_plugin_id: None,
            }),
            plugin_namespace: Some(plugin_name.clone()),
            plugin_root: Some(plugin_root.abs()),
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(outcome.errors.len(), 1);
    assert!(
        outcome.errors[0].message.contains("invalid qualified name"),
        "expected qualified name length error, got: {:?}",
        outcome.errors
    );
}

#[tokio::test]
async fn direct_child_discovery_ignores_nested_skills() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("plugin");
    let skills_root = plugin_root.join("skills");
    let direct = write_skill_at(&skills_root, "direct", "direct", "direct skill");
    write_skill_at(&skills_root, "nested/too-deep", "too-deep", "nested skill");

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: skills_root.abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: Some(PluginIdentity {
                plugin_id: "plugin@test".to_string(),
                remote_plugin_id: None,
            }),
            plugin_namespace: Some("plugin".to_string()),
            plugin_root: Some(plugin_root.abs()),
            discovery_mode: SkillDiscoveryMode::DirectChildren,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(outcome.errors.is_empty());
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "plugin:direct".to_string(),
            description: "direct skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&direct),
            scope: SkillScope::User,
            plugin_id: Some("plugin@test".to_string()),
            remote_plugin_id: None,
        }]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn direct_child_discovery_skips_skills_resolving_outside_plugin_root() {
    let root = tempfile::tempdir().expect("tempdir");
    let plugin_root = root.path().join("plugin");
    let skills_root = plugin_root.join("skills");
    let outside_root = root.path().join("outside");
    write_skill_at(&outside_root, "escaped", "escaped", "escaped skill");
    fs::create_dir_all(&skills_root).expect("create skills root");
    std::os::unix::fs::symlink(outside_root.join("escaped"), skills_root.join("escaped"))
        .expect("create skill symlink");

    let outcome = load_skills_from_roots(
        [SkillRoot {
            path: skills_root.abs(),
            scope: SkillScope::User,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: Some(PluginIdentity {
                plugin_id: "plugin@test".to_string(),
                remote_plugin_id: None,
            }),
            plugin_namespace: Some("plugin".to_string()),
            plugin_root: Some(plugin_root.abs()),
            discovery_mode: SkillDiscoveryMode::DirectChildren,
        }],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(outcome.skills.is_empty());
}

#[tokio::test]
async fn loads_short_description_from_metadata() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_dir = codex_home.path().join("skills/demo");
    fs::create_dir_all(&skill_dir).unwrap();
    let contents = "---\nname: demo-skill\ndescription: long description\nmetadata:\n  short-description: short summary\n---\n\n# Body\n";
    let skill_path = skill_dir.join(SKILLS_FILENAME);
    fs::write(&skill_path, contents).unwrap();

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "demo-skill".to_string(),
            description: "long description".to_string(),
            short_description: Some("short summary".to_string()),
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_unquoted_description_containing_colon_space() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_raw_skill_at(
        &codex_home.path().join("skills"),
        "colon-description",
        "name: colon-description\ndescription: AWS deployment patterns: ECS Fargate, Lambda, and S3",
    );

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "colon-description".to_string(),
            description: "AWS deployment patterns: ECS Fargate, Lambda, and S3".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_unquoted_short_description_containing_colon_space_and_apostrophe() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_raw_skill_at(
        &codex_home.path().join("skills"),
        "colon-short-description",
        "name: colon-short-description\ndescription: long description\nmetadata:\n  short-description: What's included: builds and tests",
    );

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "colon-short-description".to_string(),
            description: "long description".to_string(),
            short_description: Some("What's included: builds and tests".to_string()),
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn loads_unrecognized_frontmatter_fields_that_need_quotes() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_raw_skill_at(
        &codex_home.path().join("skills"),
        "repaired-unknown-fields",
        "name: repaired-unknown-fields\ndescription: valid description\nargument-hint: <duration: e.g. 7d, 2w>\ntags: [next,@supabase/ssr]",
    );

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "repaired-unknown-fields".to_string(),
            description: "valid description".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn preserves_block_scalar_body_while_repairing_other_fields() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_path = write_raw_skill_at(
        &codex_home.path().join("skills"),
        "block-description-with-repair",
        "name: block-description-with-repair\ndescription: |-\n  Build for AWS: ECS\nargument-hint: <duration: e.g. 7d>",
    );

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "block-description-with-repair".to_string(),
            description: "Build for AWS: ECS".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::User,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[tokio::test]
async fn preserves_overlong_short_descriptions() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let skill_dir = codex_home.path().join("skills/demo");
    fs::create_dir_all(&skill_dir).unwrap();
    let too_long = "x".repeat(MAX_DESCRIPTION_LEN + 1);
    let contents = format!(
        "---\nname: demo-skill\ndescription: long description\nmetadata:\n  short-description: {too_long}\n---\n\n# Body\n"
    );
    fs::write(skill_dir.join(SKILLS_FILENAME), contents).unwrap();

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(outcome.skills[0].short_description, Some(too_long));
}

#[tokio::test]
async fn skips_hidden_and_invalid() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let hidden_dir = codex_home.path().join("skills/.hidden");
    fs::create_dir_all(&hidden_dir).unwrap();
    fs::write(
        hidden_dir.join(SKILLS_FILENAME),
        "---\nname: hidden\ndescription: hidden\n---\n",
    )
    .unwrap();

    // Invalid because missing closing frontmatter.
    let invalid_dir = codex_home.path().join("skills/invalid");
    fs::create_dir_all(&invalid_dir).unwrap();
    fs::write(invalid_dir.join(SKILLS_FILENAME), "---\nname: bad").unwrap();

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert_eq!(outcome.skills.len(), 0);
    assert_eq!(outcome.errors.len(), 1);
    assert!(
        outcome.errors[0]
            .message
            .contains("missing YAML frontmatter"),
        "expected frontmatter error"
    );
}

#[tokio::test]
async fn preserves_overlong_descriptions() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let max_desc = "\u{1F4A1}".repeat(MAX_DESCRIPTION_LEN);
    write_skill(&codex_home, "max-len", "max-len", &max_desc);

    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 1);

    let too_long_desc = "\u{1F4A1}".repeat(MAX_DESCRIPTION_LEN + 1);
    write_skill(&codex_home, "too-long", "too-long", &too_long_desc);
    let outcome = load_user_skills_for_test(&codex_home).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(outcome.skills.len(), 2);
    let too_long_skill = outcome
        .skills
        .iter()
        .find(|skill| skill.name == "too-long")
        .expect("too-long skill");
    assert_eq!(too_long_skill.description, too_long_desc);
}

#[tokio::test]
async fn loads_skills_from_repo_scoped_root() {
    let repo_dir = tempfile::tempdir().expect("tempdir");

    let skills_root = repo_dir
        .path()
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    let skill_path = write_skill_at(&skills_root, "repo", "repo-skill", "from repo");

    let outcome = load_skills_for_test([local_skill_root(&skills_root, SkillScope::Repo)]).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "repo-skill".to_string(),
            description: "from repo".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::Repo,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}
#[tokio::test]
async fn loads_skills_from_multiple_repo_scoped_roots() {
    let repo_dir = tempfile::tempdir().expect("tempdir");

    let root_skills_root = repo_dir
        .path()
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    let nested_skills_root = repo_dir
        .path()
        .join("nested")
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    let root_skill_path = write_skill_at(&root_skills_root, "root", "root-skill", "from root");
    let nested_skill_path =
        write_skill_at(&nested_skills_root, "nested", "nested-skill", "from nested");

    let outcome = load_skills_for_test([
        local_skill_root(&nested_skills_root, SkillScope::Repo),
        local_skill_root(&root_skills_root, SkillScope::Repo),
    ])
    .await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "nested-skill".to_string(),
                description: "from nested".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: normalized(&nested_skill_path),
                scope: SkillScope::Repo,
                plugin_id: None,
                remote_plugin_id: None,
            },
            SkillMetadata {
                name: "root-skill".to_string(),
                description: "from root".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: normalized(&root_skill_path),
                scope: SkillScope::Repo,
                plugin_id: None,
                remote_plugin_id: None,
            },
        ]
    );
}
#[tokio::test]
async fn merges_root_results_in_input_order_when_scans_finish_out_of_order() {
    const ROOT_COUNT: usize = MAX_CONCURRENT_ROOT_SCANS + 1;

    let temp = tempfile::tempdir().expect("tempdir");
    let roots = (0..ROOT_COUNT)
        .map(|index| temp.path().join(format!("root-{index}")))
        .collect::<Vec<_>>();
    for root in &roots {
        fs::create_dir_all(root).expect("create root");
    }
    let first_skill = roots[0].join("broken/SKILL.md");
    let second_skill = roots[1].join("broken/SKILL.md");
    for (path, contents) in [
        (&first_skill, "missing frontmatter"),
        (&second_skill, "also missing frontmatter"),
    ] {
        fs::create_dir_all(path.parent().expect("skill parent")).expect("create skill directory");
        fs::write(path, contents).expect("write skill");
    }

    let blocked_walk_root = PathUri::from_abs_path(&roots[0].abs());
    let file_system = Arc::new(BlockingRepoSkillRootFileSystem {
        inner: Arc::clone(&LOCAL_FS),
        blocked_walk_root: Some(blocked_walk_root),
        blocked_walk_gate: Semaphore::new(/*permits*/ 0),
        walks_started: AtomicUsize::new(/*v*/ 0),
        walk_started: Notify::new(),
    });
    let root_file_system: Arc<dyn ExecutorFileSystem> = file_system.clone();
    let skill_roots = roots
        .iter()
        .enumerate()
        .map(|(index, root)| SkillRoot {
            path: root.abs(),
            scope: if index == 0 {
                SkillScope::Repo
            } else {
                SkillScope::User
            },
            file_system: Arc::clone(&root_file_system),
            plugin_identity: None,
            plugin_namespace: Some("test".to_string()),
            plugin_root: None,
            discovery_mode: SkillDiscoveryMode::Recursive,
        })
        .collect::<Vec<_>>();
    let root_scan_slots = Semaphore::new(MAX_CONCURRENT_ROOT_SCANS);
    let load = tokio::spawn(async move {
        crate::root_loader::load_and_merge_skill_roots(
            skill_roots,
            /*plugin_skill_snapshots*/ None,
            &root_scan_slots,
        )
        .await
    });

    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), async {
        loop {
            let started = file_system.walk_started.notified();
            if file_system.walks_started.load(Ordering::Acquire) == ROOT_COUNT {
                break;
            }
            started.await;
        }
    })
    .await
    .expect("all skill-root walks should start despite the blocked first root");
    file_system.blocked_walk_gate.add_permits(/*n*/ 1);
    let outcome = load.await.expect("skill-root load should finish");

    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(
        outcome.errors,
        vec![
            SkillError {
                path: canonicalize_path(first_skill)
                    .expect("canonical first skill")
                    .abs(),
                message: "missing YAML frontmatter delimited by ---".to_string(),
            },
            SkillError {
                path: canonicalize_path(second_skill)
                    .expect("canonical second skill")
                    .abs(),
                message: "missing YAML frontmatter delimited by ---".to_string(),
            },
        ]
    );
}

#[tokio::test]
async fn skill_root_scans_wait_for_shared_capacity() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");
    fs::create_dir_all(&root).expect("create root");
    let root_scan_slots = Semaphore::new(MAX_CONCURRENT_ROOT_SCANS);
    let held_slots = root_scan_slots
        .try_acquire_many(
            u32::try_from(MAX_CONCURRENT_ROOT_SCANS).expect("root scan limit should fit in u32"),
        )
        .expect("root scan slots should be available");
    let load = crate::root_loader::load_and_merge_skill_roots(
        [SkillRoot {
            path: root.abs(),
            scope: SkillScope::Repo,
            file_system: Arc::clone(&LOCAL_FS),
            plugin_identity: None,
            plugin_namespace: Some("test".to_string()),
            plugin_root: None,
            discovery_mode: SkillDiscoveryMode::Recursive,
        }],
        /*plugin_skill_snapshots*/ None,
        &root_scan_slots,
    );
    tokio::pin!(load);

    assert!(futures::poll!(load.as_mut()).is_pending());
    drop(held_slots);
    let outcome = load.await;

    assert_eq!(outcome.skills, Vec::new());
    assert_eq!(outcome.errors, Vec::new());
}

#[tokio::test]
async fn deduplicates_by_path_preferring_first_root() {
    let root = tempfile::tempdir().expect("tempdir");

    let skill_path = write_skill_at(root.path(), "dupe", "dupe-skill", "from repo");

    let outcome = load_skills_from_roots(
        [
            SkillRoot {
                path: root.path().abs(),
                scope: SkillScope::Repo,
                file_system: Arc::clone(&LOCAL_FS),
                plugin_identity: None,
                plugin_namespace: None,
                plugin_root: None,
                discovery_mode: SkillDiscoveryMode::Recursive,
            },
            SkillRoot {
                path: root.path().abs(),
                scope: SkillScope::User,
                file_system: Arc::clone(&LOCAL_FS),
                plugin_identity: None,
                plugin_namespace: None,
                plugin_root: None,
                discovery_mode: SkillDiscoveryMode::Recursive,
            },
        ],
        /*plugin_skill_snapshots*/ None,
        Arc::new(Semaphore::new(MAX_CONCURRENT_ROOT_SCANS)),
    )
    .await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "dupe-skill".to_string(),
            description: "from repo".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::Repo,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn deduplicated_symlinked_skill_preserves_first_discovery_path() {
    let source_root = tempfile::tempdir().expect("source tempdir");
    let first_root = tempfile::tempdir().expect("first tempdir");
    let second_root = tempfile::tempdir().expect("second tempdir");
    let skill_path = write_skill_at(source_root.path(), "demo", "demo", "shared skill");
    symlink_dir(
        &source_root.path().join("demo"),
        &first_root.path().join("first-link"),
    );
    symlink_dir(
        &source_root.path().join("demo"),
        &second_root.path().join("second-link"),
    );

    let outcome = load_skills_for_test([
        local_skill_root(first_root.path(), SkillScope::Repo),
        local_skill_root(second_root.path(), SkillScope::User),
    ])
    .await;
    let canonical_skill_path = normalized(&skill_path);
    let expected_root = normalized(first_root.path());
    let expected_discovery_path = expected_root.join("first-link/SKILL.md");

    assert_eq!(outcome.skills.len(), 1);
    assert_eq!(
        (
            outcome.skill_root_for_path(&canonical_skill_path),
            outcome.skill_discovery_path_for_path(&canonical_skill_path),
        ),
        (Some(&expected_root), Some(&expected_discovery_path))
    );
}

#[tokio::test]
async fn keeps_duplicate_names_from_repo_and_user() {
    let codex_home = tempfile::tempdir().expect("tempdir");
    let repo_dir = tempfile::tempdir().expect("tempdir");

    let user_skill_path = write_skill(&codex_home, "user", "dupe-skill", "from user");
    let repo_skills_root = repo_dir
        .path()
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    let repo_skill_path = write_skill_at(&repo_skills_root, "repo", "dupe-skill", "from repo");

    let outcome = load_skills_for_test([
        local_skill_root(&repo_skills_root, SkillScope::Repo),
        local_skill_root(&codex_home.path().join(SKILLS_DIR_NAME), SkillScope::User),
    ])
    .await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: "from repo".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: normalized(&repo_skill_path),
                scope: SkillScope::Repo,
                plugin_id: None,
                remote_plugin_id: None,
            },
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: "from user".to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: normalized(&user_skill_path),
                scope: SkillScope::User,
                plugin_id: None,
                remote_plugin_id: None,
            },
        ]
    );
}

#[tokio::test]
async fn keeps_duplicate_names_from_nested_codex_dirs() {
    let repo_dir = tempfile::tempdir().expect("tempdir");

    let root_skills_root = repo_dir
        .path()
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    let nested_skills_root = repo_dir
        .path()
        .join("nested")
        .join(REPO_ROOT_CONFIG_DIR_NAME)
        .join(SKILLS_DIR_NAME);
    let root_skill_path = write_skill_at(&root_skills_root, "root", "dupe-skill", "from root");
    let nested_skill_path =
        write_skill_at(&nested_skills_root, "nested", "dupe-skill", "from nested");

    let outcome = load_skills_for_test([
        local_skill_root(&nested_skills_root, SkillScope::Repo),
        local_skill_root(&root_skills_root, SkillScope::Repo),
    ])
    .await;

    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    let root_path = normalized(&root_skill_path);
    let nested_path = normalized(&nested_skill_path);
    let (first_path, second_path, first_description, second_description) =
        if root_path <= nested_path {
            (root_path, nested_path, "from root", "from nested")
        } else {
            (nested_path, root_path, "from nested", "from root")
        };
    assert_eq!(
        outcome.skills,
        vec![
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: first_description.to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: first_path,
                scope: SkillScope::Repo,
                plugin_id: None,
                remote_plugin_id: None,
            },
            SkillMetadata {
                name: "dupe-skill".to_string(),
                description: second_description.to_string(),
                short_description: None,
                interface: None,
                dependencies: None,
                policy: None,
                path_to_skills_md: second_path,
                scope: SkillScope::Repo,
                plugin_id: None,
                remote_plugin_id: None,
            },
        ]
    );
}
#[tokio::test]
async fn loads_skills_from_system_scoped_root() {
    let codex_home = tempfile::tempdir().expect("tempdir");

    let skill_path = write_system_skill(&codex_home, "system", "system-skill", "from system");
    let system_root = codex_home.path().join("skills/.system");

    let outcome = load_skills_for_test([local_skill_root(&system_root, SkillScope::System)]).await;
    assert!(
        outcome.errors.is_empty(),
        "unexpected errors: {:?}",
        outcome.errors
    );
    assert_eq!(
        outcome.skills,
        vec![SkillMetadata {
            name: "system-skill".to_string(),
            description: "from system".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: normalized(&skill_path),
            scope: SkillScope::System,
            plugin_id: None,
            remote_plugin_id: None,
        }]
    );
}
