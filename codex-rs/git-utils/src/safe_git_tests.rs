use super::*;
use crate::git_config::GitConfigScope;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn selected_filter_policy_allows_unused_and_rejects_selected_at_every_scope() {
    let config_dir = tempfile::tempdir().expect("config");
    let config = config_dir.path().join("global.gitconfig");
    std::fs::write(&config, "").expect("config file");

    for scope in [
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

fn filter_entries(
    scope: GitConfigScope,
    origin: &Path,
    key: &str,
    value: &str,
) -> BTreeMap<String, GitConfigEntry> {
    let origin = if origin == Path::new("command line:") {
        "command line:".to_string()
    } else {
        format!("file:{}", origin.display())
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
