use super::*;
use crate::GitReadError;
use crate::apply::ApplyGitRequest;
use crate::apply::apply_git_patch;
use crate::get_has_changes;
use crate::git_command::GitRunner;
use crate::git_config::GitConfigScope;
use crate::guarded_config::GuardedGitConfig;
#[cfg(unix)]
use crate::patch_paths::stage_paths;
#[cfg(unix)]
use crate::status_guard::prepare_status_config;
use crate::try_get_has_changes;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
#[cfg(unix)]
use std::ffi::OsStr;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::process::Output;
use tokio::process::Command as TokioCommand;

const FILTER_MARKER_KEY: &str = "codex.sentinelprobe-ran";
const FILTER_MARKER_COMMAND: &str =
    "git config codex.sentinelprobe-ran true && git hash-object --stdin";

#[test]
fn status_filter_driver_limit_is_exact() {
    let at_limit = (0..MAX_EXECUTABLE_FILTER_DRIVERS)
        .map(|index| format!("driver-{index}"))
        .collect::<BTreeSet<_>>();
    assert!(validate_executable_driver_count(&ExecutableFilterDrivers(at_limit.clone())).is_ok());

    let mut over_limit = at_limit;
    over_limit.insert("one-too-many".to_string());
    let error = validate_executable_driver_count(&ExecutableFilterDrivers(over_limit))
        .expect_err("driver after exact limit must be refused");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

fn run_git(cwd: &Path, args: &[&str]) -> Output {
    let mut command = std::process::Command::new("git");
    isolate_git_command_environment(&mut command);
    command
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run Git")
}

fn run_git_success(cwd: &Path, args: &[&str]) -> Output {
    let output = run_git(cwd, args);
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn init_filter_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().expect("tempdir");
    run_git_success(repo.path(), &["init"]);
    std::fs::write(repo.path().join("file.txt"), "content\n").expect("write file");
    repo
}

fn configure_marker_filter(cwd: &Path, driver: &str) {
    run_git_success(
        cwd,
        &[
            "config",
            &format!("filter.{driver}.clean"),
            FILTER_MARKER_COMMAND,
        ],
    );
}

fn marker_filter_ran(cwd: &Path) -> bool {
    run_git(cwd, &["config", "--get", FILTER_MARKER_KEY])
        .status
        .success()
}

#[test]
fn selected_filter_policy_allows_unused_and_rejects_selected_at_every_scope() {
    let config_dir = tempfile::tempdir().expect("config");
    let config = config_dir.path().join("global.gitconfig");
    std::fs::write(&config, "").expect("config file");

    for scope in [
        GitConfigScope::Unknown,
        GitConfigScope::System,
        GitConfigScope::Global,
        GitConfigScope::Local,
        GitConfigScope::Worktree,
        GitConfigScope::Command,
    ] {
        for (key, value) in [
            ("filter.demo.clean", "./clean.sh"),
            ("filter.lfs.clean", "git-lfs clean -- %f"),
            ("filter.lfs.smudge", "git-lfs smudge -- %f"),
            ("filter.lfs.process", "git-lfs filter-process"),
        ] {
            let entries = filter_entries(scope, &config, key, value);
            let driver = filter_driver_name(key).expect("driver name");
            let selected = BTreeMap::from([(b"file.txt".to_vec(), driver.clone())]);
            assert!(
                selected_executable_filter(&entries, &selected)
                    .expect("selected filter policy")
                    .is_some(),
                "{scope:?} {key}"
            );
            let unused = BTreeMap::from([(b"file.txt".to_vec(), "other".to_string())]);
            assert_eq!(
                selected_executable_filter(&entries, &unused).expect("unused filter policy"),
                None,
                "{scope:?} {key}"
            );
        }
    }
}

#[test]
fn selected_filter_policy_allows_effective_empty_value() {
    let disabled = filter_entries(
        GitConfigScope::Command,
        Path::new("command line:"),
        "filter.demo.clean",
        "",
    );
    let selected = BTreeMap::from([(b"file.txt".to_vec(), "demo".to_string())]);
    assert_eq!(
        selected_executable_filter(&disabled, &selected).expect("empty filter policy"),
        None
    );
}

#[test]
fn git_add_filter_policy_rejects_clean_and_process_but_allows_smudge_only() {
    let selected = BTreeMap::from([(b"file.txt".to_vec(), "demo".to_string())]);
    for (key, rejected) in [
        ("filter.demo.clean", true),
        ("filter.demo.smudge", false),
        ("filter.demo.process", true),
    ] {
        let entries = filter_entries(
            GitConfigScope::Local,
            Path::new(".git/config"),
            key,
            "codex-definitely-missing-filter-command",
        );
        assert_eq!(
            selected_executable_filter_for(&entries, &selected, FilterExecution::GitAdd)
                .expect("Git add filter policy")
                .is_some(),
            rejected,
            "{key}"
        );
    }
}

#[test]
fn selected_filter_checkin_policy_table_is_canonical() {
    for (command, required, expected) in [
        ("clean", None, SelectedFilterPolicy::Refused),
        ("process", None, SelectedFilterPolicy::Refused),
        ("smudge", None, SelectedFilterPolicy::NeedsRequiredValue),
        ("smudge", Some(false), SelectedFilterPolicy::Allowed),
        ("smudge", Some(true), SelectedFilterPolicy::Refused),
    ] {
        let entries = filter_entries(
            GitConfigScope::Local,
            Path::new(".git/config"),
            &format!("filter.demo.{command}"),
            "helper-command",
        );
        assert_eq!(
            classify_selected_filter(&entries, "demo", required),
            expected,
            "{command} required={required:?}"
        );
    }
}

#[test]
fn filter_snapshot_retains_required_without_treating_it_as_executable() {
    let mut entries = filter_entries(
        GitConfigScope::Local,
        Path::new("config"),
        "filter.demo.smudge",
        "smudge-command",
    );
    entries.extend(filter_entries(
        GitConfigScope::Command,
        Path::new("command line:"),
        "filter.demo.required",
        "true",
    ));
    assert_eq!(
        executable_filter_drivers(&entries)
            .expect("executable drivers")
            .0,
        BTreeSet::from(["demo".to_string()])
    );
    let selected = BTreeMap::from([(b"file.txt".to_vec(), "demo".to_string())]);
    assert_eq!(
        selected_executable_filter_for(&entries, &selected, FilterExecution::GitAdd)
            .expect("Git add filter policy"),
        None
    );
    assert_eq!(
        entries
            .get("filter.demo.required")
            .map(|entry| entry.value.as_str()),
        Some("true"),
    );

    let required_only = filter_entries(
        GitConfigScope::Local,
        Path::new("config"),
        "filter.demo.required",
        "true",
    );
    assert!(
        executable_filter_drivers(&required_only)
            .expect("required-only config")
            .is_empty()
    );
}

#[test]
fn filter_driver_parser_accepts_empty_name() {
    assert_eq!(
        filter_driver_name("filter..clean").expect("empty filter name"),
        ""
    );
}

#[test]
fn filter_attribute_parser_rejects_malformed_or_unexpected_records() {
    let paths = vec![b"a.txt".to_vec(), b"b.txt".to_vec()];
    let parsed =
        parse_filter_attributes(b"a.txt\0filter\0unspecified\0b.txt\0filter\0lfs\0", &paths)
            .expect("parse attributes");
    assert_eq!(
        parsed.get(b"a.txt".as_slice()),
        Some(&FilterAttributeValue::AmbiguousSentinel(
            "unspecified".to_string()
        ))
    );
    assert_eq!(
        parsed.get(b"b.txt".as_slice()),
        Some(&FilterAttributeValue::Driver("lfs".to_string()))
    );

    for output in [
        b"a.txt\0filter\0unspecified".as_slice(),
        b"a.txt\0merge\0unspecified\0b.txt\0filter\0lfs\0".as_slice(),
        b"a.txt\0filter\0unspecified\0".as_slice(),
        b"a.txt\0filter\0unspecified\0a.txt\0filter\0lfs\0".as_slice(),
    ] {
        assert!(
            parse_filter_attributes(output, &paths).is_err(),
            "{output:?}"
        );
    }
}

#[test]
fn sentinel_probe_primitives_preserve_order_budget_and_truth_table() {
    assert_eq!(
        classify_sentinel_filter_probes(
            /*required_succeeded*/ true, /*optional_succeeded*/ None,
        ),
        SentinelFilterProbeResolution::SpecialAttributeState
    );
    assert_eq!(
        classify_sentinel_filter_probes(
            /*required_succeeded*/ false, /*optional_succeeded*/ None,
        ),
        SentinelFilterProbeResolution::NeedsOptionalProbe
    );
    assert_eq!(
        classify_sentinel_filter_probes(
            /*required_succeeded*/ false,
            /*optional_succeeded*/ Some(true),
        ),
        SentinelFilterProbeResolution::LiteralDriver
    );
    assert_eq!(
        classify_sentinel_filter_probes(
            /*required_succeeded*/ false,
            /*optional_succeeded*/ Some(false),
        ),
        SentinelFilterProbeResolution::ProbeFailure
    );

    let neutralization = vec![
        "-c".to_string(),
        "include.path=/private/filter-neutralization.gitconfig".to_string(),
    ];
    assert_eq!(
        sentinel_filter_probe_config_args(&neutralization, "set", /*required*/ true)
            .expect("sentinel config args"),
        vec![
            "-c",
            "include.path=/private/filter-neutralization.gitconfig",
            "-c",
            "filter.set.required=true",
        ]
    );
    assert_eq!(
        sentinel_filter_probe_config_args(&neutralization, "ordinary", /*required*/ true)
            .expect_err("reject non-sentinel config argument")
            .kind(),
        io::ErrorKind::InvalidInput
    );

    let mut budget = SentinelFilterProbeBudget::default();
    assert_eq!(SentinelFilterProbeBudget::max_probes(), 16);
    for _ in 0..SentinelFilterProbeBudget::max_probes() {
        budget.ensure_probe_available().expect("probe in budget");
        budget.record_completed_probe();
    }
    assert_eq!(
        budget
            .ensure_probe_available()
            .expect_err("hard probe budget")
            .to_string(),
        "refusing to continue Git filter sentinel disambiguation after 16 child probes (hard limit: 16)"
    );
}

#[test]
fn sentinel_special_states_and_literal_driver_names_remain_distinct() {
    for (driver, special_rule) in [
        ("set", "file.txt filter\n"),
        ("unset", "file.txt -filter\n"),
        ("unspecified", ""),
    ] {
        let repo = init_filter_repo();
        let root = repo.path();
        configure_marker_filter(root, driver);
        let git = GitRunner::for_cwd_io(root).expect("trusted Git");
        let mut config =
            GuardedGitConfig::authorize(&git, root, Vec::new()).expect("authorized config sources");

        std::fs::write(root.join(".gitattributes"), special_rule).expect("write special attribute");
        config
            .authorize_filter_paths(&["file.txt".to_string()])
            .expect("allow special attribute state");
        assert!(!marker_filter_ran(root), "special {driver}");

        std::fs::write(
            root.join(".gitattributes"),
            format!("file.txt filter={driver}\n"),
        )
        .expect("write literal attribute");
        let mut literal_config = GuardedGitConfig::authorize(&git, root, Vec::new())
            .expect("authorize literal-attribute operation");
        let result = literal_config.authorize_filter_paths(&["file.txt".to_string()]);
        let error = match result {
            Ok(_) => panic!("accepted literal sentinel-named driver {driver}"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::Unsupported, "{driver}");
        assert!(!marker_filter_ran(root), "literal {driver}");
    }
}

#[test]
fn sentinel_probe_neutralizes_every_known_driver_after_attribute_swap() {
    for alternate_driver in ["race", "x=y"] {
        let repo = init_filter_repo();
        let root = repo.path();
        configure_marker_filter(root, "set");
        configure_marker_filter(root, alternate_driver);
        std::fs::write(root.join(".gitattributes"), "file.txt filter\n")
            .expect("write initial attribute");

        let git = GitRunner::for_cwd_io(root).expect("trusted Git");
        let config =
            GuardedGitConfig::authorize(&git, root, Vec::new()).expect("authorized config sources");
        let entries = read_filter_config(&config).expect("filter config");
        let executable_drivers = executable_filter_drivers(&entries).expect("executable drivers");
        let neutralization = config
            .build_filter_override(&executable_drivers)
            .expect("filter neutralization");
        let output = run_git_success(root, &["check-attr", "-z", "filter", "--", "file.txt"]);
        let attributes = parse_filter_attributes(&output.stdout, &[b"file.txt".to_vec()])
            .expect("initial attribute snapshot");

        std::fs::write(
            root.join(".gitattributes"),
            format!("file.txt filter={alternate_driver}\n"),
        )
        .expect("swap attribute after snapshot");
        let resolved = resolve_filter_attribute_sentinels(
            &config,
            attributes,
            &executable_drivers,
            &neutralization,
        )
        .expect("resolve stale sentinel snapshot safely");
        assert!(resolved.is_empty(), "{alternate_driver}");
        assert!(!marker_filter_ran(root), "{alternate_driver}");
    }
}

#[test]
fn high_cardinality_ordinary_sentinels_stop_at_hard_child_probe_budget() {
    let repo = init_filter_repo();
    let root = repo.path();
    configure_marker_filter(root, "unspecified");
    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    let config =
        GuardedGitConfig::authorize(&git, root, Vec::new()).expect("authorized config sources");
    let entries = read_filter_config(&config).expect("filter config");
    let executable_drivers = executable_filter_drivers(&entries).expect("executable drivers");
    let neutralization = config
        .build_filter_override(&executable_drivers)
        .expect("filter neutralization");
    let attributes = (0..=SentinelFilterProbeBudget::max_probes())
        .map(|index| {
            (
                format!("ordinary-{index}.txt").into_bytes(),
                FilterAttributeValue::AmbiguousSentinel("unspecified".to_string()),
            )
        })
        .collect();

    let error = resolve_filter_attribute_sentinels(
        &config,
        attributes,
        &executable_drivers,
        &neutralization,
    )
    .expect_err("refuse sentinel work beyond hard child-probe budget");
    assert_eq!(
        error.to_string(),
        "refusing to continue Git filter sentinel disambiguation after 16 child probes (hard limit: 16)"
    );
    assert!(!marker_filter_ran(root));
}

#[test]
fn sentinel_probe_rejects_non_sentinel_driver_before_launch() {
    let repo = init_filter_repo();
    let root = repo.path();
    configure_marker_filter(root, "set");
    std::fs::write(root.join(".gitattributes"), "file.txt filter\n")
        .expect("write special attribute");
    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    let config =
        GuardedGitConfig::authorize(&git, root, Vec::new()).expect("authorized config sources");
    let entries = read_filter_config(&config).expect("filter config");
    let executable_drivers = executable_filter_drivers(&entries).expect("executable drivers");
    let neutralization = config
        .build_filter_override(&executable_drivers)
        .expect("filter neutralization");
    let mut budget = SentinelFilterProbeBudget::default();

    let error = sentinel_spelling_selects_filter_driver(
        &config,
        b"file.txt",
        "ordinary",
        &neutralization,
        &mut budget,
    )
    .expect_err("reject non-sentinel probe");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(!marker_filter_ran(root));
}

fn filter_entries(
    scope: GitConfigScope,
    origin: &Path,
    key: &str,
    value: &str,
) -> BTreeMap<String, GitConfigEntry> {
    let origin = if origin == Path::new("command line:") {
        crate::git_config::GitConfigOrigin::CommandLine
    } else {
        crate::git_config::GitConfigOrigin::File(origin.to_path_buf())
    };
    BTreeMap::from([(
        key.to_string(),
        GitConfigEntry {
            scope,
            origin,
            key: key.to_string(),
            value: value.to_string(),
        },
    )])
}

#[cfg(unix)]
fn run_isolated_test(test_name: &str, env: &[(&str, &OsStr)]) {
    let mut command = std::process::Command::new(std::env::current_exe().expect("test binary"));
    isolate_git_command_environment(&mut command);
    command
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD", "1")
        .env("RUST_TEST_THREADS", "1");
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("run isolated test process");
    assert!(
        output.status.success(),
        "isolated test {test_name} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn run_git_async(repo_path: &Path, args: &[&str]) {
    let output = TokioCommand::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .await
        .expect("run git command");
    assert!(
        output.status.success(),
        "git command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn create_test_git_repo(temp_dir: &tempfile::TempDir) -> std::path::PathBuf {
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path).expect("create repo dir");
    run_git_async(&repo_path, &["init"]).await;
    run_git_async(&repo_path, &["config", "user.name", "Test User"]).await;
    run_git_async(&repo_path, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(repo_path.join("test.txt"), "test content").expect("write test file");
    run_git_async(&repo_path, &["add", "."]).await;
    run_git_async(&repo_path, &["commit", "-m", "initial"]).await;
    repo_path
}

#[tokio::test]
async fn ordinary_apply_allows_an_unselected_executable_filter() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    std::fs::write(repo_path.join("test.txt"), "old\n").expect("write fixture");
    run_git_async(&repo_path, &["add", "test.txt"]).await;
    run_git_async(&repo_path, &["commit", "-m", "normalize fixture"]).await;
    run_git_async(
        &repo_path,
        &[
            "config",
            "filter.unused.clean",
            "codex-definitely-missing-filter-command",
        ],
    )
    .await;

    let result = apply_git_patch(&ApplyGitRequest {
        cwd: repo_path.clone(),
        diff: "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n"
            .to_string(),
        revert: false,
        preflight: false,
    })
    .expect("unused filter must not block apply");
    assert_eq!(result.exit_code, 0);
    let contents = std::fs::read_to_string(repo_path.join("test.txt")).expect("read result");
    assert!(
        matches!(contents.as_str(), "new\n" | "new\r\n"),
        "expected the patched contents with a platform line ending, got {contents:?}"
    );
}

async fn configure_clean_filter(repo_path: &Path, tracked_path: &str) {
    std::fs::write(
        repo_path.join(".gitattributes"),
        format!("{tracked_path} filter=x=y\n"),
    )
    .expect("write attributes");
    run_git_async(repo_path, &["add", ".gitattributes"]).await;
    run_git_async(repo_path, &["commit", "-m", "attributes"]).await;
    run_git_async(
        repo_path,
        &[
            "config",
            "filter.x=y.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    )
    .await;

    let tracked_file = repo_path.join(tracked_path);
    let contents = std::fs::read_to_string(&tracked_file).expect("read tracked file");
    std::thread::sleep(std::time::Duration::from_secs(1));
    std::fs::write(tracked_file, contents).expect("refresh tracked file");
}

async fn configured_filter_ran(repo_path: &Path) -> bool {
    let output = TokioCommand::new("git")
        .args(["config", "--get", "codex.filterran"])
        .current_dir(repo_path)
        .output()
        .await
        .expect("read filter marker");
    output.status.success()
}

async fn add_submodule_with_clean_filter(parent: &Path) {
    let source = tempfile::tempdir().expect("submodule source");
    let source_path = source.path();
    run_git_async(source_path, &["init"]).await;
    run_git_async(source_path, &["config", "user.name", "Test User"]).await;
    run_git_async(source_path, &["config", "user.email", "test@example.com"]).await;
    std::fs::write(source_path.join("nested.txt"), "original\n").expect("nested file");
    std::fs::write(
        source_path.join(".gitattributes"),
        "nested.txt filter=codex-test\n",
    )
    .expect("nested attributes");
    run_git_async(source_path, &["add", "."]).await;
    run_git_async(source_path, &["commit", "-m", "seed"]).await;

    run_git_async(
        parent,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            source_path.to_str().expect("source path"),
            "nested",
        ],
    )
    .await;
    run_git_async(parent, &["commit", "-m", "add submodule"]).await;
    let nested = parent.join("nested");
    run_git_async(
        &nested,
        &[
            "config",
            "filter.codex-test.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    )
    .await;
    std::fs::write(nested.join("nested.txt"), "modified\n").expect("dirty nested file");
}

#[tokio::test]
async fn get_has_changes_rejects_configured_clean_filter_without_running_it() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    configure_clean_filter(&repo_path, "test.txt").await;

    assert_eq!(get_has_changes(&repo_path).await, None);
    assert_eq!(
        try_get_has_changes(&repo_path).await,
        Err(GitReadError::SelectedExecutableFilter {
            driver: "x=y".to_string(),
            path: "test.txt".to_string(),
        })
    );
    assert!(!configured_filter_ran(&repo_path).await);
}

#[tokio::test]
async fn get_has_changes_distinguishes_filter_sentinels_from_literal_driver_names() {
    for (driver, sentinel_attribute) in
        [("set", "filter"), ("unset", "-filter"), ("unspecified", "")]
    {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;
        let clean_key = format!("filter.{driver}.clean");
        run_git_async(
            &repo_path,
            &[
                "config",
                &clean_key,
                "git config codex.filterran true && git hash-object --stdin",
            ],
        )
        .await;

        let sentinel = if sentinel_attribute.is_empty() {
            String::new()
        } else {
            format!("test.txt {sentinel_attribute}\n")
        };
        std::fs::write(repo_path.join(".gitattributes"), sentinel)
            .expect("write sentinel attribute");
        run_git_async(&repo_path, &["add", ".gitattributes"]).await;
        run_git_async(&repo_path, &["commit", "-m", "sentinel attribute"]).await;

        assert_eq!(try_get_has_changes(&repo_path).await, Ok(false), "{driver}");
        assert!(!configured_filter_ran(&repo_path).await, "{driver}");

        std::fs::write(
            repo_path.join(".gitattributes"),
            format!("test.txt filter={driver}\n"),
        )
        .expect("write literal sentinel-named driver");
        assert_eq!(
            try_get_has_changes(&repo_path).await,
            Err(GitReadError::SelectedExecutableFilter {
                driver: driver.to_string(),
                path: "test.txt".to_string(),
            }),
            "{driver}"
        );
        assert!(!configured_filter_ran(&repo_path).await, "{driver}");
    }
}

#[tokio::test]
async fn checked_has_changes_distinguishes_non_repository() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    assert_eq!(
        try_get_has_changes(temp_dir.path()).await,
        Err(GitReadError::NotRepository {
            path: std::fs::canonicalize(temp_dir.path()).expect("canonical temp dir"),
        })
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn checked_has_changes_accepts_non_utf8_repository_root() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = temp_dir
        .path()
        .join(OsString::from_vec(b"repo-\xff".to_vec()));
    std::fs::create_dir(&repo_path).expect("create non-UTF-8 repository directory");
    run_git_async(&repo_path, &["init"]).await;

    assert_eq!(try_get_has_changes(&repo_path).await, Ok(false));
}

#[cfg(unix)]
#[tokio::test]
async fn checked_has_changes_preserves_lexical_repository_ancestry_for_git_selection() {
    if std::env::var_os("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD").is_none() {
        let fixture = tempfile::tempdir().expect("fixture");
        let outer = fixture.path().join("outer");
        let physical_nested = fixture.path().join("physical-nested");
        let lexical_nested = outer.join("nested");
        let outer_bin = outer.join("bin");
        std::fs::create_dir_all(&outer_bin).expect("create outer repository bin");
        std::fs::create_dir_all(&physical_nested).expect("create nested repository");
        run_git_async(&outer, &["init", "-q"]).await;
        run_git_async(&physical_nested, &["init", "-q"]).await;
        std::os::unix::fs::symlink(&physical_nested, &lexical_nested)
            .expect("symlink nested repository");

        let native_false = std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
            .find_map(|directory| {
                let candidate = std::fs::canonicalize(directory.join("false")).ok()?;
                crate::git_executable::is_native_executable_file(&candidate).then_some(candidate)
            })
            .expect("native false executable");
        let outer_git = outer_bin.join("git");
        std::fs::copy(native_false, &outer_git).expect("copy native false as outer Git");
        assert!(
            crate::git_executable::is_native_executable_file(&outer_git),
            "decoy Git must pass production native-executable selection"
        );

        let search_path =
            std::env::join_paths([outer_bin.as_path()]).expect("construct single-candidate PATH");
        run_isolated_test(
            "safe_git::tests::checked_has_changes_preserves_lexical_repository_ancestry_for_git_selection",
            &[
                ("CODEX_GIT_UTILS_TARGET_REPO", lexical_nested.as_os_str()),
                ("CODEX_GIT_UTILS_PHYSICAL_REPO", physical_nested.as_os_str()),
                ("PATH", search_path.as_os_str()),
            ],
        );
        return;
    }

    let physical_nested = PathBuf::from(
        std::env::var_os("CODEX_GIT_UTILS_PHYSICAL_REPO").expect("physical repository"),
    );
    assert_eq!(
        try_get_has_changes(&physical_nested).await,
        Err(GitReadError::CommandFailed {
            operation: "statusFilterPreparation".to_string(),
            exit_code: None,
        }),
        "the canonical spelling must select and fail on the first eligible native decoy"
    );
    let lexical_nested =
        PathBuf::from(std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"));
    assert_eq!(
        try_get_has_changes(&lexical_nested).await,
        Err(GitReadError::NoTrustedGit),
        "the lexical spelling must exclude the enclosing repository's native Git decoy"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn root_probe_distinguishes_repository_command_failure() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    let failing_git = temp_dir.path().join("failing-git");
    let resolved_git = std::process::Command::new("/bin/sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("resolve real Git");
    assert!(resolved_git.status.success(), "resolve real Git");
    let real_git = format!(
        "'{}'",
        String::from_utf8(resolved_git.stdout)
            .expect("Git path UTF-8")
            .trim()
            .replace('\'', "'\\''")
    );
    std::fs::write(
        &failing_git,
        format!(
            "#!/bin/sh\ncase \" $* \" in *\" rev-parse --show-toplevel \"*) exit 42 ;; esac\nexec {real_git} \"$@\"\n"
        ),
    )
    .expect("write failing Git");
    let mut permissions = std::fs::metadata(&failing_git)
        .expect("read failing Git metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&failing_git, permissions).expect("mark failing Git executable");

    let git = GitRunner::from_executable_for_test(&repo_path, failing_git)
        .expect("authority-bound failing Git");
    assert_eq!(
        prepare_status_config(&git, &repo_path).await.map(|_| ()),
        Err(GitReadError::CommandFailed {
            operation: "resolveGitRoot".to_string(),
            exit_code: Some(42),
        })
    );
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_git_without_show_scope_uses_origin_fallback() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    std::fs::write(repo_path.join(".gitattributes"), "test.txt filter=legacy\n")
        .expect("attributes");
    run_git_async(&repo_path, &["add", ".gitattributes"]).await;
    run_git_async(&repo_path, &["commit", "-m", "attributes"]).await;
    run_git_async(
        &repo_path,
        &["config", "filter.legacy.clean", "git hash-object --stdin"],
    )
    .await;

    let output = std::process::Command::new("/bin/sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("resolve fixture Git");
    assert!(output.status.success(), "resolve fixture Git");
    let real_git = String::from_utf8(output.stdout)
        .expect("Git path UTF-8")
        .trim()
        .to_string();
    let wrapper = temp_dir.path().join("legacy-git");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  if [ \"$arg\" = --show-scope ]; then\n    exit 129\n  fi\ndone\nexec '{}' \"$@\"\n",
            real_git.replace('\'', "'\\''")
        ),
    )
    .expect("legacy Git wrapper");
    let mut permissions = std::fs::metadata(&wrapper)
        .expect("legacy Git metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&wrapper, permissions).expect("legacy Git executable");

    let git = GitRunner::from_executable_for_test(&repo_path, wrapper)
        .expect("authority-bound legacy Git");
    assert_eq!(
        prepare_status_config(&git, &repo_path).await.map(|_| ()),
        Err(GitReadError::SelectedExecutableFilter {
            driver: "legacy".to_string(),
            path: "test.txt".to_string(),
        })
    );
}

#[tokio::test]
async fn get_has_changes_rejects_core_worktree_redirection_before_running_filter() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    let redirected = temp_dir.path().join("redirected");
    std::fs::create_dir(&redirected).expect("redirected worktree");
    std::fs::write(redirected.join(".gitattributes"), "test.txt filter=x=y\n").expect("attributes");
    std::fs::write(redirected.join("test.txt"), "redirected content\n").expect("redirected file");
    run_git_async(
        &repo_path,
        &[
            "config",
            "core.worktree",
            redirected.to_str().expect("redirected path"),
        ],
    )
    .await;
    run_git_async(
        &repo_path,
        &[
            "config",
            "filter.x=y.clean",
            "git config codex.filterran true && git hash-object --stdin",
        ],
    )
    .await;

    assert_eq!(get_has_changes(&repo_path).await, None);
    assert_eq!(
        try_get_has_changes(&repo_path).await,
        Err(GitReadError::RepositoryRootMismatch {
            expected_root: std::fs::canonicalize(&repo_path).expect("canonical repository"),
            reported_root: std::fs::canonicalize(&redirected).expect("canonical redirection"),
        })
    );
    assert!(!configured_filter_ran(&repo_path).await);
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_git_config_cannot_hide_a_selected_local_filter_from_probe() {
    if std::env::var_os("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD").is_none() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;
        std::fs::write(repo_path.join(".gitattributes"), "test.txt filter=evil\n")
            .expect("attributes");
        run_git_async(&repo_path, &["add", ".gitattributes"]).await;
        run_git_async(&repo_path, &["commit", "-m", "attributes"]).await;

        let marker = temp_dir.path().join("filter-ran");
        let filter = repo_path.join("clean.sh");
        std::fs::write(
            &filter,
            format!("#!/bin/sh\n: > '{}'\ncat\n", marker.display()),
        )
        .expect("filter script");
        let mut permissions = std::fs::metadata(&filter)
            .expect("filter metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&filter, permissions).expect("filter executable");
        run_git_async(
            &repo_path,
            &[
                "config",
                "--local",
                "filter.evil.clean",
                filter.to_str().expect("filter path"),
            ],
        )
        .await;
        std::fs::write(repo_path.join("test.txt"), "changed\n").expect("modify tracked file");

        let safe_config = temp_dir.path().join("safe.gitconfig");
        std::fs::write(&safe_config, "").expect("safe config");
        run_isolated_test(
            "safe_git::tests::legacy_git_config_cannot_hide_a_selected_local_filter_from_probe",
            &[
                ("CODEX_GIT_UTILS_TARGET_REPO", repo_path.as_os_str()),
                ("GIT_CONFIG", safe_config.as_os_str()),
            ],
        );
        assert!(!marker.exists(), "selected local filter must not run");
        return;
    }

    let repo_path =
        PathBuf::from(std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"));
    assert_eq!(get_has_changes(&repo_path).await, None);
}

#[tokio::test]
async fn get_has_changes_does_not_enter_dirty_submodules() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    add_submodule_with_clean_filter(&repo_path).await;

    assert_eq!(get_has_changes(&repo_path).await, Some(false));
    assert!(!configured_filter_ran(&repo_path.join("nested")).await);
}

#[cfg(unix)]
#[tokio::test]
async fn apply_and_stage_reject_global_relative_filter_without_running_it() {
    if std::env::var_os("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD").is_none() {
        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(&global_config, "[filter \"evil\"]\n\tclean = ./clean.sh\n")
            .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        run_isolated_test(
            "safe_git::tests::apply_and_stage_reject_global_relative_filter_without_running_it",
            &[
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
            ],
        );
        return;
    }

    use std::os::unix::fs::PermissionsExt;

    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let repo_path = create_test_git_repo(&temp_dir).await;
    let marker = repo_path.join("filter-ran");
    std::fs::write(repo_path.join("test.txt"), "old\n").expect("tracked file");
    std::fs::write(repo_path.join(".gitattributes"), "test.txt filter=evil\n").expect("attributes");
    std::fs::write(
        repo_path.join("clean.sh"),
        format!("#!/bin/sh\ntouch '{}'\ncat\n", marker.display()),
    )
    .expect("relative filter");
    let mut permissions = std::fs::metadata(repo_path.join("clean.sh"))
        .expect("filter metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(repo_path.join("clean.sh"), permissions).expect("filter executable");
    run_git_async(
        &repo_path,
        &[
            "-c",
            "filter.evil.clean=",
            "add",
            "test.txt",
            ".gitattributes",
        ],
    )
    .await;
    run_git_async(
        &repo_path,
        &["-c", "filter.evil.clean=", "commit", "-m", "fixture"],
    )
    .await;
    assert!(!marker.exists(), "setup must not run filter");

    let diff = "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let error = apply_git_patch(&ApplyGitRequest {
        cwd: repo_path.clone(),
        diff: diff.to_string(),
        revert: false,
        preflight: false,
    })
    .expect_err("reject relative global filter");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(!marker.exists(), "apply must not run filter");
    assert_eq!(
        std::fs::read_to_string(repo_path.join("test.txt")).expect("read tracked file"),
        "old\n"
    );

    let error = stage_paths(&repo_path, diff).expect_err("reject filter during staging");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(!marker.exists(), "staging must not run filter");
}

#[cfg(unix)]
#[tokio::test]
async fn nested_cwd_rejects_global_lfs_filter_without_running_it() {
    if std::env::var_os("CODEX_GIT_UTILS_SAFE_GIT_ENV_CHILD").is_none() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let repo_path = create_test_git_repo(&temp_dir).await;
        let nested = repo_path.join("nested");
        let repo_bin = repo_path.join("bin");
        std::fs::create_dir(&nested).expect("nested cwd");
        std::fs::create_dir(&repo_bin).expect("repository bin");
        std::fs::write(repo_path.join(".gitattributes"), "test.txt filter=lfs\n")
            .expect("attributes");
        run_git_async(&repo_path, &["add", ".gitattributes"]).await;
        run_git_async(&repo_path, &["commit", "-m", "attributes"]).await;
        std::fs::write(repo_path.join("test.txt"), "changed\n").expect("modify tracked file");

        let config_dir = tempfile::tempdir().expect("config tempdir");
        let global_config = config_dir.path().join("global.gitconfig");
        let system_config = config_dir.path().join("system.gitconfig");
        std::fs::write(
            &global_config,
            "[filter \"lfs\"]\n\tclean = git-lfs clean -- %f\n",
        )
        .expect("write global config");
        std::fs::write(&system_config, "").expect("write system config");
        let marker = config_dir.path().join("repo-lfs-ran");
        let primary_git_marker = config_dir.path().join("repo-primary-git-ran");
        let repo_git_lfs = repo_bin.join("git-lfs");
        std::fs::write(
            &repo_git_lfs,
            "#!/bin/sh\n: > \"$CODEX_GIT_UTILS_UNSAFE_LFS_MARKER\"\nwhile IFS= read -r line\ndo\n  printf '%s\\n' \"$line\"\ndone\n",
        )
        .expect("repository git-lfs");
        let mut permissions = std::fs::metadata(&repo_git_lfs)
            .expect("repository git-lfs metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&repo_git_lfs, permissions)
            .expect("make repository git-lfs executable");

        let output = std::process::Command::new("/bin/sh")
            .args(["-c", "command -v git"])
            .output()
            .expect("resolve git executable");
        assert!(output.status.success(), "resolve git executable");
        let git_path = PathBuf::from(
            String::from_utf8(output.stdout)
                .expect("Git path UTF-8")
                .trim(),
        );
        let repo_git = repo_bin.join("git");
        std::fs::write(
            &repo_git,
            "#!/bin/sh\nprintf ran > \"$CODEX_GIT_UTILS_PRIMARY_GIT_MARKER\"\nexec \"$CODEX_GIT_UTILS_REAL_GIT\" \"$@\"\n",
        )
        .expect("repository Git");
        let mut permissions = std::fs::metadata(&repo_git)
            .expect("repository Git metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&repo_git, permissions).expect("make repository Git executable");
        let search_path = std::env::join_paths([
            repo_bin.as_path(),
            git_path.parent().expect("Git executable directory"),
        ])
        .expect("construct controlled PATH");
        run_isolated_test(
            "safe_git::tests::nested_cwd_rejects_global_lfs_filter_without_running_it",
            &[
                ("CODEX_GIT_UTILS_TARGET_REPO", repo_path.as_os_str()),
                ("CODEX_GIT_UTILS_UNSAFE_LFS_MARKER", marker.as_os_str()),
                (
                    "CODEX_GIT_UTILS_PRIMARY_GIT_MARKER",
                    primary_git_marker.as_os_str(),
                ),
                ("CODEX_GIT_UTILS_REAL_GIT", git_path.as_os_str()),
                ("GIT_CONFIG_GLOBAL", global_config.as_os_str()),
                ("GIT_CONFIG_SYSTEM", system_config.as_os_str()),
                ("PATH", search_path.as_os_str()),
            ],
        );
        assert!(!marker.exists(), "repository git-lfs must not run");
        assert!(
            !primary_git_marker.exists(),
            "repository-controlled primary Git must not run"
        );
        return;
    }

    let repo_path =
        PathBuf::from(std::env::var_os("CODEX_GIT_UTILS_TARGET_REPO").expect("target repository"));
    let diff = "diff --git a/test.txt b/test.txt\n--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n";
    let error = apply_git_patch(&ApplyGitRequest {
        cwd: repo_path.join("nested"),
        diff: diff.to_string(),
        revert: false,
        preflight: false,
    })
    .expect_err("reject global Git LFS filter from nested cwd");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);

    let error = stage_paths(&repo_path, diff).expect_err("reject global Git LFS during staging");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
}
