use super::*;
use crate::git_config::GitConfigScope;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Output;

const FILTER_MARKER_KEY: &str = "codex.sentinelprobe-ran";
const FILTER_MARKER_COMMAND: &str =
    "git config codex.sentinelprobe-ran true && git hash-object --stdin";

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
        executable_filter_drivers(&entries).expect("executable drivers"),
        BTreeSet::from(["demo".to_string()])
    );
    let neutralization = GitFilterNeutralization {
        git_config_args: Vec::new(),
        _config_dir: None,
        filter_config: entries,
    };
    assert_eq!(
        neutralization.filter_value("demo", "required"),
        Some("true")
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

        std::fs::write(root.join(".gitattributes"), special_rule).expect("write special attribute");
        ensure_no_selected_executable_git_filters(&git, root, &["file.txt".to_string()], &[])
            .expect("allow special attribute state");
        assert!(!marker_filter_ran(root), "special {driver}");

        std::fs::write(
            root.join(".gitattributes"),
            format!("file.txt filter={driver}\n"),
        )
        .expect("write literal attribute");
        let result =
            ensure_no_selected_executable_git_filters(&git, root, &["file.txt".to_string()], &[]);
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
        let entries = read_filter_config(&git, root, &[]).expect("filter config");
        let executable_drivers = executable_filter_drivers(&entries).expect("executable drivers");
        let neutralization = executable_filter_guard(&git, root, entries, &executable_drivers)
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
            &git,
            root,
            attributes,
            &[],
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
    let entries = read_filter_config(&git, root, &[]).expect("filter config");
    let executable_drivers = executable_filter_drivers(&entries).expect("executable drivers");
    let neutralization = executable_filter_guard(&git, root, entries, &executable_drivers)
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
        &git,
        root,
        attributes,
        &[],
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
fn unrelated_sentinel_probe_failures_remain_generic_and_fail_closed() {
    let repo = init_filter_repo();
    let root = repo.path();
    configure_marker_filter(root, "set");
    std::fs::write(root.join(".gitattributes"), "file.txt filter\n")
        .expect("write special attribute");
    let git = GitRunner::for_cwd_io(root).expect("trusted Git");
    let entries = read_filter_config(&git, root, &[]).expect("filter config");
    let executable_drivers = executable_filter_drivers(&entries).expect("executable drivers");
    let neutralization = executable_filter_guard(&git, root, entries, &executable_drivers)
        .expect("filter neutralization");
    let mut budget = SentinelFilterProbeBudget::default();
    let malformed_config = ["-c".to_string(), "=".to_string()];

    let error = sentinel_spelling_selects_filter_driver(
        &git,
        root,
        b"file.txt",
        "set",
        &malformed_config,
        &neutralization,
        &mut budget,
    )
    .expect_err("malformed config must fail both probes");
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert!(
        error
            .to_string()
            .starts_with("git filter attribute selection probe failed with required status")
    );
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
