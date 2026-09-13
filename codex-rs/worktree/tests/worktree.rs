#![allow(clippy::expect_used)]

use codex_worktree::CreateWorktree;
use codex_worktree::ManagedWorktree;
use codex_worktree::WorktreeManager;
use codex_worktree::WorktreeSettings;
use codex_worktree::default_worktree_base;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

struct RepositoryFixture {
    _temp_dir: TempDir,
    codex_home: PathBuf,
    repository: PathBuf,
}

impl RepositoryFixture {
    fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("create temporary test directory");
        let temp_root = dunce::canonicalize(temp_dir.path()).expect("canonicalize temporary root");
        let codex_home = temp_root.join("codex-home");
        let repository = temp_root.join("project");
        fs::create_dir_all(&codex_home).expect("create Codex home");
        initialize_repository(&repository);

        Self {
            _temp_dir: temp_dir,
            codex_home,
            repository,
        }
    }

    fn manager(&self) -> WorktreeManager {
        let settings =
            WorktreeSettings::from_desktop_config(&self.codex_home, /*desktop*/ None)
                .expect("load default worktree settings");
        WorktreeManager::new(settings)
    }
}

fn initialize_repository(repository: &Path) {
    fs::create_dir_all(repository.join("nested/component"))
        .expect("create temporary repository directories");
    run_git(repository, &["init", "--quiet"]);
    fs::write(
        repository.join("README.md"),
        "initial repository contents\n",
    )
    .expect("write tracked repository file");
    fs::write(
        repository.join("nested/component/tracked.txt"),
        "nested tracked file\n",
    )
    .expect("write nested tracked repository file");
    run_git(repository, &["add", "."]);
    commit(repository, "initial commit");
}

fn commit(repository: &Path, message: &str) {
    run_git(
        repository,
        &["commit", "--quiet", "--no-gpg-sign", "-m", message],
    );
}

fn create_worktree(
    manager: &WorktreeManager,
    source_cwd: &Path,
    base: Option<&str>,
) -> anyhow::Result<ManagedWorktree> {
    manager.create(&CreateWorktree {
        source_cwd: source_cwd.to_path_buf(),
        base: base.map(str::to_owned),
    })
}

#[test]
fn removing_managed_worktree_refuses_dirty_or_unrelated_checkouts() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let checkout = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("create managed checkout");
    let other = RepositoryFixture::new();
    assert!(manager.remove(&other.repository, &checkout.root).is_err());
    assert!(
        manager
            .remove(&fixture.repository, &fixture.repository)
            .is_err()
    );
    let aliased_current = checkout
        .root
        .join("..")
        .join(checkout.root.file_name().expect("managed checkout name"));
    assert!(
        manager
            .remove(&aliased_current, &checkout.root)
            .expect_err("cannot remove current checkout via alias")
            .to_string()
            .contains("current worktree")
    );
    assert!(checkout.root.exists());
    fs::write(checkout.root.join("unsaved.txt"), "local work\n").expect("write untracked file");
    assert!(manager.remove(&fixture.repository, &checkout.root).is_err());
    assert!(checkout.root.join("unsaved.txt").exists());

    fs::remove_file(checkout.root.join("unsaved.txt")).expect("remove untracked file");
    fs::write(fixture.repository.join(".git/info/exclude"), ".env\n")
        .expect("ignore local settings file");
    fs::write(checkout.root.join(".env"), "local settings\n").expect("write ignored file");
    assert!(
        manager
            .remove(&fixture.repository, &checkout.root)
            .expect_err("refuse ignored local files")
            .to_string()
            .contains("ignored local files")
    );
    assert!(checkout.root.join(".env").exists());
    fs::remove_file(checkout.root.join(".env")).expect("remove ignored file");
    manager
        .remove(&fixture.repository, &checkout.root)
        .expect("remove clean checkout");
    assert!(!checkout.root.exists());
    assert!(
        manager
            .list(&fixture.repository)
            .expect("list remaining")
            .is_empty()
    );
}

fn git_output(repository: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .current_dir(repository)
        .args([
            "-c",
            "user.name=Codex Worktree Test",
            "-c",
            "user.email=codex-worktree-test@example.invalid",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "tag.gpgSign=false",
        ])
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run git {args:?} in {repository:?}: {error}"))
}

fn run_git(repository: &Path, args: &[&str]) -> String {
    let output = git_output(repository, args);
    assert!(
        output.status.success(),
        "git {args:?} failed in {repository:?}: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_owned()
}

#[test]
fn cli_creation_shares_default_and_custom_desktop_pools_without_enabling_cleanup() {
    let fixture = RepositoryFixture::new();
    for root in [None, Some(fixture.codex_home.join("custom-desktop"))] {
        let desktop =
            root.map(|root| HashMap::from([("git-worktree-root".to_owned(), json!(root))]));
        let desktop_settings =
            WorktreeSettings::from_desktop_config(&fixture.codex_home, desktop.as_ref())
                .expect("resolve desktop settings");
        let mut cli_desktop = desktop.unwrap_or_default();
        cli_desktop.insert("worktree-auto-cleanup-enabled".to_owned(), json!("unused"));
        cli_desktop.insert("worktree-keep-count".to_owned(), json!(0));
        let manager = WorktreeManager::new(
            WorktreeSettings::for_cli(&fixture.codex_home, Some(&cli_desktop))
                .expect("resolve CLI settings"),
        );
        let worktree = create_worktree(&manager, &fixture.repository, /*base*/ None)
            .expect("create CLI worktree");
        assert!(worktree.root.starts_with(&desktop_settings.root));
        assert_eq!(
            manager.settings(),
            &WorktreeSettings {
                auto_cleanup_enabled: false,
                ..desktop_settings.clone()
            }
        );
        assert_eq!(
            WorktreeManager::new(desktop_settings)
                .list(&fixture.repository)
                .expect("list desktop worktrees"),
            vec![worktree],
        );
    }
}

#[test]
fn thread_binding_rejects_primary_and_unmanaged_worktrees() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    fs::create_dir_all(&manager.settings().root).expect("create managed root");
    assert!(
        manager
            .bind_thread(&fixture.repository, "not-a-managed-thread")
            .is_err(),
    );
    assert!(
        !fixture.repository.join(".git/codex-thread.json").exists(),
        "primary checkout ownership metadata must not be modified",
    );

    let unmanaged = manager.settings().root.join("scratch").join("project");
    fs::create_dir_all(unmanaged.parent().expect("unmanaged parent"))
        .expect("create unmanaged parent");
    run_git(
        &fixture.repository,
        &[
            "worktree",
            "add",
            "--detach",
            unmanaged.to_str().expect("UTF-8 unmanaged worktree"),
            "HEAD",
        ],
    );
    assert!(manager.bind_thread(&unmanaged, "unmanaged").is_err());
    assert!(manager.owner(&unmanaged).is_err());
}

#[test]
fn thread_binding_writes_the_exact_desktop_owner_schema_into_git_metadata() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let checkout = manager.settings().root.join("a1b2/project");
    fs::create_dir_all(checkout.parent().expect("checkout parent")).expect("create bucket");
    run_git(
        &fixture.repository,
        &[
            "worktree",
            "add",
            "--detach",
            checkout.to_str().expect("UTF-8 checkout"),
            "HEAD",
        ],
    );
    let thread_id = "019f1234-5678-7000-8000-000000000001";

    assert_eq!(manager.owner(&checkout).expect("read initial owner"), None);

    manager
        .bind_thread(&checkout, thread_id)
        .expect("bind managed worktree to thread");

    assert_eq!(
        manager.owner(&checkout).expect("read bound owner"),
        Some(thread_id.to_owned()),
    );

    let metadata_path = checkout.join(run_git(
        &checkout,
        &["rev-parse", "--git-path", "codex-thread.json"],
    ));
    let metadata: Value = serde_json::from_slice(
        &fs::read(&metadata_path).expect("read Desktop-compatible worktree owner metadata"),
    )
    .expect("parse Desktop-compatible worktree owner metadata");

    assert_eq!(
        metadata,
        json!({
            "version": 1,
            "ownerThreadId": thread_id,
        }),
    );
}

#[test]
fn creation_uses_desktop_hash_bucket_layout_and_detached_head() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let source_head = run_git(&fixture.repository, &["rev-parse", "HEAD"]);

    let worktree = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("create managed worktree");

    assert_eq!(
        manager
            .settings()
            .root
            .join(".metadata_never_index")
            .is_file(),
        cfg!(target_os = "macos"),
    );
    assert_eq!(
        worktree.root.parent().and_then(Path::parent),
        Some(fixture.codex_home.join("worktrees").as_path())
    );
    assert_eq!(
        worktree.root.file_name(),
        fixture.repository.file_name(),
        "Desktop expects the repository name underneath the hash bucket",
    );

    let bucket = worktree
        .root
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .expect("worktree has a UTF-8 hash-bucket directory");
    assert_eq!(bucket.len(), 4);
    assert!(bucket.bytes().all(|byte| byte.is_ascii_hexdigit()));

    assert_eq!(worktree.cwd, worktree.root);
    assert_eq!(worktree.source_root, fixture.repository);
    assert_eq!(worktree.source_cwd, worktree.source_root);
    assert_eq!(worktree.head_sha, source_head);
    assert_eq!(worktree.branch, None);
    assert_eq!(run_git(&worktree.root, &["rev-parse", "HEAD"]), source_head);
    assert!(
        !git_output(&worktree.root, &["symbolic-ref", "--quiet", "HEAD"])
            .status
            .success(),
        "Desktop-managed worktrees start with a detached HEAD",
    );
}

#[test]
fn creation_preserves_the_source_working_directory_relative_to_the_repository() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    fs::create_dir_all(&manager.settings().root).expect("precreate managed worktree root");
    let source_cwd = fixture.repository.join("nested/component");

    let worktree = create_worktree(&manager, &source_cwd, /*base*/ None)
        .expect("create worktree from nested source directory");

    assert_eq!(worktree.source_root, fixture.repository);
    assert_eq!(worktree.source_cwd, source_cwd);
    assert_eq!(worktree.cwd, worktree.root.join("nested/component"));
    assert!(worktree.cwd.join("tracked.txt").is_file());
    assert_eq!(
        manager
            .settings()
            .root
            .join(".metadata_never_index")
            .is_file(),
        cfg!(target_os = "macos"),
    );
}

#[test]
fn creation_respects_an_explicit_starting_commit() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let initial_commit = run_git(&fixture.repository, &["rev-parse", "HEAD"]);

    fs::write(fixture.repository.join("second.txt"), "second commit\n")
        .expect("write another tracked repository file");
    run_git(&fixture.repository, &["add", "second.txt"]);
    commit(&fixture.repository, "second commit");

    let worktree = create_worktree(&manager, &fixture.repository, Some(&initial_commit))
        .expect("create worktree from explicit commit");

    assert_eq!(worktree.head_sha, initial_commit);
    assert_eq!(
        run_git(&worktree.root, &["rev-parse", "HEAD"]),
        initial_commit
    );
    assert!(!worktree.root.join("second.txt").exists());
}

#[test]
fn creation_peels_annotated_tags_to_the_checked_out_commit() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let commit = run_git(&fixture.repository, &["rev-parse", "HEAD"]);
    run_git(
        &fixture.repository,
        &["tag", "-a", "release", "-m", "annotated release"],
    );

    let worktree = create_worktree(&manager, &fixture.repository, Some("release"))
        .expect("create worktree from annotated tag");

    assert_eq!(worktree.head_sha, commit);
    assert_eq!(run_git(&worktree.root, &["rev-parse", "HEAD"]), commit);
}

#[cfg(unix)]
#[test]
fn creation_rejects_a_working_directory_that_escapes_through_a_base_symlink() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let original = run_git(&fixture.repository, &["rev-parse", "HEAD"]);
    let outside = fixture.codex_home.join("outside-worktree");
    fs::create_dir_all(&outside).expect("create outside directory");
    let linked_directory = fixture.repository.join("nested/component");
    fs::remove_dir_all(&linked_directory).expect("remove existing tracked directory");
    std::os::unix::fs::symlink(&outside, &linked_directory)
        .expect("replace tracked directory with an escaping symlink");
    run_git(&fixture.repository, &["add", "-A"]);
    commit(
        &fixture.repository,
        "introduce escaping working-directory symlink",
    );
    let unsafe_revision = run_git(&fixture.repository, &["rev-parse", "HEAD"]);
    run_git(&fixture.repository, &["reset", "--hard", &original]);

    let registered_before = run_git(&fixture.repository, &["worktree", "list", "--porcelain"]);

    assert!(
        create_worktree(&manager, &linked_directory, Some(&unsafe_revision)).is_err(),
        "an escaping destination symlink must never become the thread cwd",
    );
    assert_eq!(
        run_git(&fixture.repository, &["worktree", "list", "--porcelain"]),
        registered_before
    );
}

#[test]
fn creation_ignores_inherited_git_environment() {
    const CHILD_ROOT: &str = "CODEX_WORKTREE_ENV_TEST_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        let source = root.join("project");
        let source_cwd = source.join("nested/component");
        let settings =
            WorktreeSettings::from_desktop_config(&root.join("codex-home"), /*desktop*/ None)
                .expect("fixture operation succeeds");
        let base = default_worktree_base(&source_cwd).expect("resolve source default");
        assert_eq!(base, "refs/remotes/origin/source-default");
        let worktree = create_worktree(&WorktreeManager::new(settings), &source_cwd, Some(&base))
            .expect("fixture operation succeeds");
        assert_eq!(worktree.source_root, source);
        assert_eq!(worktree.source_cwd, source_cwd);
        assert_eq!(
            worktree.head_sha,
            std::env::var("CODEX_WORKTREE_ENV_TEST_HEAD").expect("fixture operation succeeds")
        );
        return;
    }

    let fixture = RepositoryFixture::new();
    let other = fixture.codex_home.join("other-repository");
    initialize_repository(&other);
    fs::write(other.join("other.txt"), "different repository").expect("fixture operation succeeds");
    run_git(&other, &["add", "."]);
    commit(&other, "different repository");
    fs::write(
        fixture.repository.join(".gitattributes"),
        "*.txt filter=review\n",
    )
    .expect("fixture operation succeeds");
    run_git(&fixture.repository, &["add", "."]);
    commit(&fixture.repository, "checkout filter attributes");
    for (repository, branch) in [
        (&fixture.repository, "source-default"),
        (&other, "other-default"),
    ] {
        let target = format!("refs/remotes/origin/{branch}");
        run_git(repository, &["update-ref", &target, "HEAD"]);
        run_git(
            repository,
            &["symbolic-ref", "refs/remotes/origin/HEAD", &target],
        );
    }
    let mut child = Command::new(std::env::current_exe().expect("current test executable"));
    child
        .args(["--exact", "creation_ignores_inherited_git_environment", "--nocapture"])
        .env(CHILD_ROOT, fixture.repository.parent().expect("fixture parent"))
        .env("CODEX_WORKTREE_ENV_TEST_HEAD", run_git(&fixture.repository, &["rev-parse", "HEAD"]))
        .env("GIT_DIR", other.join(".git"))
        .env("GIT_COMMON_DIR", other.join(".git"))
        .env("GIT_CEILING_DIRECTORIES", &fixture.repository)
        .env("GIT_WORK_TREE", &fixture.repository)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "filter.review.required")
        .env("GIT_CONFIG_VALUE_0", "true")
        .env("GIT_CONFIG_PARAMETERS", "'filter.review.process=codex-worktree-nonexistent-filter' 'filter.review.required=true'");
    let output = child.output().expect("fixture operation succeeds");
    assert!(
        output.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn creation_disables_filters_from_destination_conditional_config() {
    let fixture = RepositoryFixture::new();
    fs::write(
        fixture.repository.join(".gitattributes"),
        "*.txt filter=late text eol=lf\n",
    )
    .expect("write checkout attributes");
    run_git(&fixture.repository, &["add", "."]);
    commit(&fixture.repository, "checkout attributes");
    let config = fixture.codex_home.join("destination.gitconfig");
    fs::write(
        &config,
        "[filter \"late\"]\nsmudge = codex-worktree-nonexistent-filter\nrequired = true\n",
    )
    .expect("write conditional filter config");
    let git_dir = fixture.repository.join(".git/worktrees");
    let condition = format!(
        "includeIf.gitdir:{}/.path",
        git_dir.to_string_lossy().replace('\\', "/")
    );
    run_git(
        &fixture.repository,
        &["config", &condition, config.to_str().expect("UTF-8 config")],
    );

    let worktree = create_worktree(&fixture.manager(), &fixture.repository, /*base*/ None)
        .expect("checkout must not invoke the destination-only filter");
    assert_eq!(
        fs::read_to_string(worktree.root.join("nested/component/tracked.txt"))
            .expect("read checked out file"),
        "nested tracked file\n",
    );
}

#[test]
fn creation_preserves_dirty_source_with_shared_git_config() {
    let fixture = RepositoryFixture::new();
    initialize_repository(&fixture.repository.join("submodule"));
    run_git(
        &fixture.repository,
        &["submodule", "add", "./submodule", "submodule"],
    );
    commit(&fixture.repository, "add initialized submodule");

    for (key, value) in [
        (
            "core.worktree",
            fixture.repository.to_str().expect("UTF-8 repository"),
        ),
        ("extensions.worktreeConfig", "true"),
        ("core.autocrlf", "false"),
        ("submodule.recurse", "true"),
    ] {
        run_git(&fixture.repository, &["config", key, value]);
    }
    let config_path = fixture.repository.join(".git/config");
    let source_config = fs::read(&config_path).expect("read shared Git config");
    let path = "nested/component/tracked.txt";
    let original = fs::read(fixture.repository.join(path)).expect("read committed file");
    let dirty = b"uncommitted source contents";
    fs::write(fixture.repository.join(path), dirty).expect("dirty the source checkout");

    let worktree = create_worktree(&fixture.manager(), &fixture.repository, /*base*/ None)
        .expect("create without resetting the source checkout");

    assert_eq!(
        (
            fs::read(fixture.repository.join(path)).expect("read dirty source file"),
            fs::read(worktree.root.join(path)).expect("read new checkout file"),
            fs::read(config_path).expect("reread shared Git config"),
        ),
        (dirty.to_vec(), original, source_config),
    );
    assert_eq!(
        (
            PathBuf::from(run_git(&worktree.cwd, &["rev-parse", "--show-toplevel"])),
            run_git(&worktree.cwd, &["status", "--porcelain"]),
        ),
        (worktree.root, String::new()),
    );
}

#[test]
fn creation_rolls_back_when_the_base_lacks_the_source_subdirectory() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let older_commit = run_git(&fixture.repository, &["rev-parse", "HEAD"]);
    let source_cwd = fixture.repository.join("introduced/later");
    fs::create_dir_all(&source_cwd).expect("create newly introduced source directory");
    fs::write(source_cwd.join("tracked.txt"), "new directory\n")
        .expect("write newly introduced tracked file");
    run_git(&fixture.repository, &["add", "."]);
    commit(&fixture.repository, "introduce source directory");

    let registered_before = run_git(&fixture.repository, &["worktree", "list", "--porcelain"]);

    assert!(
        create_worktree(&manager, &source_cwd, Some(&older_commit)).is_err(),
        "a destination cwd missing from the requested base must fail",
    );
    assert_eq!(
        run_git(&fixture.repository, &["worktree", "list", "--porcelain"]),
        registered_before
    );
    assert_eq!(
        fs::read_dir(&manager.settings().root)
            .expect("read managed root")
            .filter(|entry| {
                entry
                    .as_ref()
                    .expect("read managed root entry")
                    .file_type()
                    .expect("read managed root entry type")
                    .is_dir()
            })
            .count(),
        0,
        "failed creation must remove its own empty bucket",
    );
}

#[test]
fn creation_rejects_relative_roots_before_resolving_the_source() {
    let directory = TempDir::new().expect("create temporary directory");
    let source = directory.path().join("missing-source");
    let settings = WorktreeSettings::from_desktop_config(
        Path::new("relative-codex-home"),
        /*desktop*/ None,
    )
    .expect("load settings with a relative default root");
    let error = create_worktree(&WorktreeManager::new(settings), &source, /*base*/ None)
        .expect_err("relative roots must fail before source discovery");

    assert_eq!(
        error.to_string(),
        "managed worktree root must be an absolute path",
    );
}

#[test]
fn creation_fails_for_a_directory_outside_a_git_repository() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let non_repository = fixture.codex_home.join("not-a-repository");
    fs::create_dir_all(&non_repository).expect("create non-repository directory");

    assert!(
        create_worktree(&manager, &non_repository, /*base*/ None).is_err(),
        "managed worktree creation requires a Git repository",
    );
}

#[test]
fn listing_rejects_a_primary_checkout_even_when_its_path_matches_the_layout() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let primary = manager.settings().root.join("a1b2").join("ordinary");
    initialize_repository(&primary);

    assert!(
        manager
            .list(&primary)
            .expect("list managed worktrees for primary checkout")
            .is_empty(),
        "a primary checkout under a matching path is not a managed linked worktree",
    );

    let git_dir = fixture.codex_home.join("separate-git-dir");
    run_git(
        &primary,
        &[
            "init",
            "--separate-git-dir",
            git_dir.to_str().expect("UTF-8 fixture path"),
        ],
    );
    assert!(primary.join(".git").is_file());
    assert!(
        manager
            .list(&primary)
            .expect("fixture operation succeeds")
            .is_empty()
    );
}

#[test]
fn listing_skips_worktrees_without_a_safe_source_working_directory() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let source_cwd = fixture.repository.join("nested/component");
    let valid =
        create_worktree(&manager, &source_cwd, /*base*/ None).expect("fixture operation succeeds");
    let missing =
        create_worktree(&manager, &source_cwd, /*base*/ None).expect("fixture operation succeeds");
    let file =
        create_worktree(&manager, &source_cwd, /*base*/ None).expect("fixture operation succeeds");
    fs::remove_dir_all(&missing.cwd).expect("fixture operation succeeds");
    fs::remove_dir_all(&file.cwd).expect("fixture operation succeeds");
    fs::write(&file.cwd, "not a directory").expect("fixture operation succeeds");
    #[cfg(unix)]
    {
        let escaping = create_worktree(&manager, &source_cwd, /*base*/ None)
            .expect("fixture operation succeeds");
        fs::remove_dir_all(&escaping.cwd).expect("fixture operation succeeds");
        std::os::unix::fs::symlink(&source_cwd, &escaping.cwd).expect("fixture operation succeeds");
    }

    assert_eq!(
        manager
            .list(&source_cwd)
            .expect("fixture operation succeeds"),
        vec![valid]
    );
}

#[test]
fn listing_rejects_a_stale_registration_reused_by_another_repository() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let stale = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("fixture operation succeeds");
    fs::remove_dir_all(&stale.root).expect("fixture operation succeeds");
    let other = fixture.codex_home.join("other-repository");
    initialize_repository(&other);
    run_git(
        &other,
        &[
            "worktree",
            "add",
            "--detach",
            stale.root.to_str().expect("UTF-8 fixture path"),
            "HEAD",
        ],
    );

    assert!(
        manager
            .list(&fixture.repository)
            .expect("fixture operation succeeds")
            .is_empty()
    );
    assert_eq!(
        manager
            .list(&other)
            .expect("fixture operation succeeds")
            .len(),
        1
    );
}

#[test]
fn listing_rejects_a_stale_registration_reused_by_the_same_repository() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let original = run_git(&fixture.repository, &["rev-parse", "HEAD"]);
    fs::write(fixture.repository.join("second.txt"), "second commit\n")
        .expect("write second revision");
    run_git(&fixture.repository, &["add", "second.txt"]);
    commit(&fixture.repository, "second commit");
    let current = run_git(&fixture.repository, &["rev-parse", "HEAD"]);
    let stale = manager.settings().root.join("a1b2/project");
    let moved = manager.settings().root.join("c3d4/project");
    // Use ordinary Git checkouts: manager.create pins core.worktree to its path.
    for (path, revision) in [(&stale, &original), (&moved, &current)] {
        fs::create_dir_all(path.parent().expect("worktree bucket"))
            .expect("create worktree bucket");
        run_git(
            &fixture.repository,
            &[
                "worktree",
                "add",
                "--detach",
                path.to_str().expect("UTF-8 fixture path"),
                revision,
            ],
        );
    }
    let valid = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("create unaffected worktree");
    fs::remove_dir_all(&stale).expect("remove stale checkout");
    fs::rename(&moved, &stale).expect("reuse stale path without repairing registration");

    assert_eq!(
        manager.list(&fixture.repository).expect("list worktrees"),
        vec![valid],
    );
}

#[cfg(unix)]
#[test]
fn listing_rejects_checkout_and_bucket_aliases_but_allows_a_root_alias() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let stale = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("create stale checkout");
    fs::write(fixture.repository.join("second.txt"), "second commit\n")
        .expect("write second revision");
    run_git(&fixture.repository, &["add", "second.txt"]);
    commit(&fixture.repository, "second commit");
    let valid = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("create valid checkout");
    let expected = vec![valid.clone()];

    fs::remove_dir_all(&stale.root).expect("remove stale checkout");
    std::os::unix::fs::symlink(&valid.root, &stale.root).expect("alias stale checkout");
    assert_eq!(
        manager.list(&fixture.repository).expect("list worktrees"),
        expected,
    );

    fs::remove_file(&stale.root).expect("remove checkout alias");
    let stale_bucket = stale.root.parent().expect("stale worktree bucket");
    fs::remove_dir(stale_bucket).expect("remove stale bucket");
    std::os::unix::fs::symlink(
        valid.root.parent().expect("valid worktree bucket"),
        stale_bucket,
    )
    .expect("alias stale bucket");
    assert_eq!(
        manager.list(&fixture.repository).expect("list worktrees"),
        expected,
    );

    let aliased_root = fixture.codex_home.join("aliased-worktrees");
    std::os::unix::fs::symlink(&manager.settings().root, &aliased_root)
        .expect("alias managed root");
    let aliased_manager = WorktreeManager::new(WorktreeSettings {
        root: aliased_root,
        ..manager.settings().clone()
    });
    assert_eq!(
        aliased_manager
            .list(&fixture.repository)
            .expect("list worktrees through managed-root alias"),
        expected,
    );
}

#[cfg(unix)]
#[test]
fn listing_preserves_inventory_when_a_branch_name_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;

    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let worktree = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("fixture operation succeeds");
    let branch = b"refs/heads/non-utf8-\xff";
    // Packed refs can represent these names even on filesystems that reject
    // non-UTF-8 filenames, including APFS.
    let mut packed_ref = format!("{} ", worktree.head_sha).into_bytes();
    packed_ref.extend_from_slice(branch);
    packed_ref.push(b'\n');
    fs::write(fixture.repository.join(".git/packed-refs"), packed_ref)
        .expect("fixture operation succeeds");
    let output = Command::new("git")
        .current_dir(&worktree.root)
        .args(["symbolic-ref", "HEAD"])
        .arg(std::ffi::OsStr::from_bytes(branch))
        .output()
        .expect("fixture operation succeeds");
    assert!(output.status.success());

    assert_eq!(
        manager
            .list(&fixture.repository)
            .expect("fixture operation succeeds"),
        vec![worktree]
    );
    // The primary checkout is not managed, but its metadata is parsed too.
    fs::write(
        fixture.repository.join(".git/HEAD"),
        [b"ref: ".as_slice(), branch, b"\n"].concat(),
    )
    .expect("fixture operation succeeds");
    assert_eq!(
        manager
            .list(&fixture.repository)
            .expect("fixture operation succeeds")
            .len(),
        1
    );
}

#[test]
fn listing_only_includes_managed_worktrees_for_the_requested_repository() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let first = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("create first worktree");
    let second = create_worktree(&manager, &fixture.repository, /*base*/ None)
        .expect("create second worktree");

    let other_repository = fixture.codex_home.join("other-project");
    initialize_repository(&other_repository);
    let unrelated = create_worktree(&manager, &other_repository, /*base*/ None)
        .expect("create unrelated repository worktree");

    let listed = manager
        .list(&fixture.repository)
        .expect("list managed worktrees for source repository");
    let listed_roots: Vec<&Path> = listed.iter().map(|entry| entry.root.as_path()).collect();

    assert_eq!(listed_roots.len(), 2);
    assert!(listed_roots.contains(&first.root.as_path()));
    assert!(listed_roots.contains(&second.root.as_path()));
    assert!(!listed_roots.contains(&unrelated.root.as_path()));

    let unrelated_list = manager
        .list(&other_repository)
        .expect("list managed worktrees for the other repository");
    assert_eq!(unrelated_list.len(), 1);
    assert_eq!(unrelated_list[0].root, unrelated.root);
}

#[test]
fn listing_ignores_worktrees_outside_the_desktop_bucket_layout() {
    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();
    let root = manager.settings().root.join("scratch").join("project");
    fs::create_dir_all(root.parent().expect("scratch worktree parent"))
        .expect("create scratch worktree parent");
    run_git(
        &fixture.repository,
        &[
            "worktree",
            "add",
            "--detach",
            root.to_str().expect("UTF-8 scratch worktree"),
            "HEAD",
        ],
    );

    assert!(
        manager
            .list(&fixture.repository)
            .expect("list managed worktrees")
            .is_empty(),
        "non-Desktop worktrees must never enter the managed inventory",
    );
}

#[cfg(unix)]
#[test]
fn creation_listing_and_thread_binding_preserve_native_repository_paths() {
    use std::ffi::OsString;
    #[cfg(not(target_os = "macos"))]
    use std::os::unix::ffi::OsStringExt;

    let fixture = RepositoryFixture::new();
    let manager = fixture.manager();

    for name in [
        OsString::from("project with trailing space "),
        OsString::from("project-東京"),
        // APFS only supports creating filenames with valid UTF-8.
        #[cfg(not(target_os = "macos"))]
        OsString::from_vec(b"project-\xff".to_vec()),
    ] {
        let repository = fixture.codex_home.join(&name);
        initialize_repository(&repository);
        let worktree = create_worktree(&manager, &repository, /*base*/ None)
            .expect("create managed worktree with a native repository path");

        assert_eq!(worktree.source_root, repository);
        assert_eq!(worktree.root.file_name(), Some(name.as_os_str()));
        let listed = manager
            .list(&repository)
            .expect("list managed worktrees with a native repository path");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].root, worktree.root);

        manager
            .bind_thread(&worktree.root, "native-path-thread")
            .expect("bind managed worktree with a native repository path");
        assert_eq!(
            manager
                .owner(&worktree.root)
                .expect("read owner with a native repository path"),
            Some("native-path-thread".to_owned()),
        );
    }
}
