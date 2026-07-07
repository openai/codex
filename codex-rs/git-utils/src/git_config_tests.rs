use super::*;

#[test]
fn parses_effective_last_value_with_scope_and_origin() {
    let output = b"global\0file:/tmp/global\0filter.demo.clean\ngit-lfs clean -- %f\0\
local\0file:/repo/.git/config\0filter.demo.clean\n\0\
command\0command line:\0merge.Name.driver\nhelper %A %B\0";

    let entries = parse_effective_config(output).expect("parse config");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries.get("filter.demo.clean"),
        Some(&GitConfigEntry {
            scope: GitConfigScope::Local,
            origin: GitConfigOrigin::File("/repo/.git/config".into()),
            key: "filter.demo.clean".to_string(),
            value: String::new(),
        })
    );
    assert_eq!(
        entries.get("merge.Name.driver"),
        Some(&GitConfigEntry {
            scope: GitConfigScope::Command,
            origin: GitConfigOrigin::CommandLine,
            key: "merge.Name.driver".to_string(),
            value: "helper %A %B".to_string(),
        })
    );
}

#[test]
fn typed_config_parser_distinguishes_implicit_from_explicit_empty() {
    let scoped = b"local\0file:/repo/.git/config\0core.sharedrepository\0\
local\0file:/repo/.git/config\0core.other\n\0";
    let parsed = parse_config_value_entries(scoped).expect("parse typed scoped config");
    assert_eq!(parsed[0].key, "core.sharedrepository");
    assert_eq!(parsed[0].value, GitConfigValue::Implicit);
    assert_eq!(parsed[1].key, "core.other");
    assert_eq!(parsed[1].value, GitConfigValue::Explicit(String::new()));

    let legacy = b"file:/repo/.git/config\0core.sharedrepository\0\
file:/repo/.git/config\0core.other\n\0";
    let parsed =
        parse_config_value_entries_with_origins(legacy).expect("parse typed legacy config");
    assert_eq!(parsed[0].value, GitConfigValue::Implicit);
    assert_eq!(parsed[1].value, GitConfigValue::Explicit(String::new()));
}

#[test]
fn ordinary_config_parser_still_rejects_implicit_values() {
    for output in [
        b"local\0file:/repo/.git/config\0filter.demo.clean\0".as_slice(),
        b"file:/repo/.git/config\0include.path\0".as_slice(),
    ] {
        let result = if output.starts_with(b"local\0") {
            parse_config_entries(output)
        } else {
            parse_config_entries_with_origins(output)
        };
        assert!(result.is_err(), "implicit value unexpectedly accepted");
    }
}

#[test]
fn typed_config_parser_splits_only_the_first_value_delimiter() {
    let output = b"local\0file:/repo/.git/config\0core.demo\nfirst\nsecond\0";
    let parsed = parse_config_value_entries(output).expect("parse multiline explicit value");
    assert_eq!(
        parsed[0].value,
        GitConfigValue::Explicit("first\nsecond".to_string())
    );
}

#[test]
fn fixed_merge_config_reader_preserves_ordered_typed_records() {
    use std::io::Write as _;

    let repo = tempfile::tempdir().expect("repo");
    let root = repo.path();
    let mut init = std::process::Command::new("git");
    crate::safe_git::isolate_git_command_environment(&mut init);
    let output = init
        .args(["init", "-q"])
        .current_dir(root)
        .output()
        .expect("initialize repo");
    assert!(output.status.success());
    let mut config = std::fs::OpenOptions::new()
        .append(true)
        .open(root.join(".git/config"))
        .expect("open local config");
    write!(
        config,
        "\n[merge \"codex-reader\"]\n\tname\n\tname = later\n\tarbitrary\n\
         [merge \"codex.reader.dotted\"]\n\tunknown\n\
         [merge]\n\tcodexscalar = ignored\n"
    )
    .expect("append merge config records");
    drop(config);

    let git = GitRunner::for_cwd_io(root).expect("runner");
    let all_records = read_merge_config_records_with_fallback(&git, root, &[])
        .expect("read fixed merge config records");
    assert!(
        all_records
            .iter()
            .all(|record| record.key != "merge.codexscalar"),
        "top-level merge scalar must not be returned as a user namespace"
    );
    let records = all_records
        .into_iter()
        .filter(|record| {
            record.key.starts_with("merge.codex-reader.")
                || record.key.starts_with("merge.codex.reader.dotted.")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        records,
        vec![
            MergeConfigRecord {
                key: "merge.codex-reader.name".to_string(),
                value: GitConfigValue::Implicit,
            },
            MergeConfigRecord {
                key: "merge.codex-reader.name".to_string(),
                value: GitConfigValue::Explicit("later".to_string()),
            },
            MergeConfigRecord {
                key: "merge.codex-reader.arbitrary".to_string(),
                value: GitConfigValue::Implicit,
            },
            MergeConfigRecord {
                key: "merge.codex.reader.dotted.unknown".to_string(),
                value: GitConfigValue::Implicit,
            },
        ]
    );
}

#[test]
fn effective_shared_repository_reader_preserves_implicit_and_empty_values() {
    use std::io::Write as _;

    for (line, expected) in [
        ("\tsharedRepository\n", GitConfigValue::Implicit),
        (
            "\tsharedRepository =\n",
            GitConfigValue::Explicit(String::new()),
        ),
    ] {
        let repo = tempfile::tempdir().expect("repo");
        let root = repo.path();
        let mut init = std::process::Command::new("git");
        crate::safe_git::isolate_git_command_environment(&mut init);
        let initialized = init
            .args(["init", "-q"])
            .current_dir(root)
            .output()
            .expect("initialize repo");
        assert!(
            initialized.status.success(),
            "initialize repo: {}",
            String::from_utf8_lossy(&initialized.stderr)
        );
        let mut config = std::fs::OpenOptions::new()
            .append(true)
            .open(root.join(".git/config"))
            .expect("open local config");
        write!(config, "\n[core]\n{line}").expect("append shared-repository value");
        drop(config);

        let git = GitRunner::for_cwd_io(root).expect("Git runner");
        assert_eq!(
            read_effective_shared_repository_with_fallback(&git, root, &[])
                .expect("read shared repository"),
            Some(expected),
            "config line {line:?}"
        );
    }
}

#[test]
fn git_boolean_accepts_native_git_spellings() {
    for (value, expected) in [
        (b"true".as_slice(), true),
        (b"TRUE".as_slice(), true),
        (b"yes".as_slice(), true),
        (b"On".as_slice(), true),
        (b"1".as_slice(), true),
        (b"2".as_slice(), true),
        (b"-1".as_slice(), true),
        (b"+1".as_slice(), true),
        (b"01".as_slice(), true),
        (b"0x1".as_slice(), true),
        (b"0Xf".as_slice(), true),
        (b"010".as_slice(), true),
        (b"1k".as_slice(), true),
        (b"1M".as_slice(), true),
        (b"1g".as_slice(), true),
        (b" 1".as_slice(), true),
        (b"2147483647".as_slice(), true),
        (b"-2147483648".as_slice(), true),
        (b"-2g".as_slice(), true),
        (b"false".as_slice(), false),
        (b"FALSE".as_slice(), false),
        (b"no".as_slice(), false),
        (b"Off".as_slice(), false),
        (b"".as_slice(), false),
        (b"0".as_slice(), false),
        (b"-0".as_slice(), false),
        (b"+0".as_slice(), false),
        (b"00".as_slice(), false),
        (b"0x0".as_slice(), false),
        (b"0k".as_slice(), false),
    ] {
        assert_eq!(parse_git_boolean(value), Some(expected), "value {value:?}");
    }
}

#[test]
fn git_boolean_rejects_values_native_git_rejects() {
    for value in [
        b"08".as_slice(),
        b"-08".as_slice(),
        b"2147483648".as_slice(),
        b"-2147483649".as_slice(),
        b"9223372036854775807".as_slice(),
        b"2g".as_slice(),
        b"-3g".as_slice(),
        b"0x".as_slice(),
        b"0b1".as_slice(),
        b"1kb".as_slice(),
        b"1foo".as_slice(),
        b"1_0".as_slice(),
        b"1 ".as_slice(),
        b" ".as_slice(),
        b"+".as_slice(),
        b"-".as_slice(),
        b"not-a-bool".as_slice(),
        b"\xff".as_slice(),
    ] {
        assert_eq!(parse_git_boolean(value), None, "value {value:?}");
    }
}

#[test]
fn symmetric_git_boolean_parser_excludes_only_int_min_spellings() {
    for value in [
        b"-2147483648".as_slice(),
        b"-0x80000000".as_slice(),
        b"-020000000000".as_slice(),
        b"-2097152k".as_slice(),
        b"-2048m".as_slice(),
        b"-2g".as_slice(),
        b" -2G".as_slice(),
    ] {
        assert_eq!(parse_git_boolean(value), Some(true), "value {value:?}");
        assert_eq!(
            parse_git_boolean_symmetric_i32(value),
            None,
            "value {value:?}"
        );
    }

    for value in [
        b"0x1".as_slice(),
        b"010".as_slice(),
        b"1k".as_slice(),
        b"-1g".as_slice(),
        b" 1".as_slice(),
        b"-2147483647".as_slice(),
    ] {
        assert_eq!(
            parse_git_boolean_symmetric_i32(value),
            parse_git_boolean(value),
            "value {value:?}"
        );
    }
}

#[test]
fn entry_parsers_preserve_duplicate_include_directives_in_order() {
    let scoped = b"local\0file:.git/config\0include.path\n../unsafe.gitconfig\0\
local\0file:.git/config\0include.path\n/absolute/external.gitconfig\0";
    let legacy = b"file:.git/config\0include.path\n../unsafe.gitconfig\0\
file:.git/config\0include.path\n/absolute/external.gitconfig\0";
    let expected_scoped = vec![
        GitConfigEntry {
            scope: GitConfigScope::Local,
            origin: GitConfigOrigin::File(".git/config".into()),
            key: "include.path".to_string(),
            value: "../unsafe.gitconfig".to_string(),
        },
        GitConfigEntry {
            scope: GitConfigScope::Local,
            origin: GitConfigOrigin::File(".git/config".into()),
            key: "include.path".to_string(),
            value: "/absolute/external.gitconfig".to_string(),
        },
    ];
    assert_eq!(
        parse_config_entries(scoped).expect("scoped entries"),
        expected_scoped
    );
    assert_eq!(
        parse_config_entries_with_origins(legacy).expect("legacy entries"),
        expected_scoped
    );
}

#[cfg(unix)]
#[test]
fn config_origin_parser_preserves_non_utf8_unix_path_bytes() {
    use std::os::unix::ffi::OsStrExt;

    let entries = parse_config_entries(b"local\0file:/tmp/config-\xff\0include.path\n/tmp/safe\0")
        .expect("non-UTF-8 config origin");
    let GitConfigOrigin::File(path) = &entries[0].origin else {
        panic!("expected file origin");
    };
    assert_eq!(path.as_os_str().as_bytes(), b"/tmp/config-\xff");
}

#[test]
fn rejects_malformed_config_records() {
    for output in [
        b"global\0file:/tmp/config\0key\nvalue".as_slice(),
        b"global\0file:/tmp/config\0key\0".as_slice(),
        b"mystery\0file:/tmp/config\0key\nvalue\0".as_slice(),
        b"global\0\0key\nvalue\0".as_slice(),
        b"global\0file:/tmp/config\0\nvalue\0".as_slice(),
    ] {
        assert!(parse_config_entries(output).is_err(), "{output:?}");
    }
}

#[test]
fn parses_legacy_origin_records_in_effective_order() {
    let output = b"file:/tmp/global\0filter.demo.clean\nfirst\0\
file:/repo/.git/config\0filter.demo.clean\n\0\
command line:\0merge.Name.driver\nhelper %A %B\0";
    let entries = parse_effective_config_with_origins(output).expect("parse legacy config");
    assert_eq!(
        entries.get("filter.demo.clean"),
        Some(&GitConfigEntry {
            scope: GitConfigScope::Local,
            origin: GitConfigOrigin::File("/repo/.git/config".into()),
            key: "filter.demo.clean".to_string(),
            value: String::new(),
        })
    );
    assert_eq!(
        entries.get("merge.Name.driver"),
        Some(&GitConfigEntry {
            scope: GitConfigScope::Command,
            origin: GitConfigOrigin::CommandLine,
            key: "merge.Name.driver".to_string(),
            value: "helper %A %B".to_string(),
        })
    );
}

#[test]
fn path_containment_uses_component_boundaries() {
    let root = Path::new("/repo/root");
    assert!(path_is_within(Path::new("/repo/root"), root));
    assert!(path_is_within(Path::new("/repo/root/config"), root));
    assert!(!path_is_within(Path::new("/repo/rooted/config"), root));
    assert!(!path_is_within(Path::new("/repo"), root));
}

#[test]
fn git_normalizes_case_insensitive_key_parts_before_effective_parsing() {
    let mut command = std::process::Command::new("git");
    crate::safe_git::isolate_git_command_environment(&mut command);
    let output = command
        .args([
            "-c",
            "FiLtEr.DeMo.ClEaN=first",
            "-c",
            "filter.DeMo.clean=second",
            "config",
            "--null",
            "--show-scope",
            "--show-origin",
            "--get-regexp",
            r"^filter\.DeMo\.clean$",
        ])
        .output()
        .expect("query mixed-case Git config");
    assert!(
        output.status.success(),
        "Git config query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let entries = parse_effective_config(&output.stdout).expect("parse Git-normalized config");
    assert_eq!(entries.len(), 1);
    let entry = entries
        .get("filter.DeMo.clean")
        .expect("canonical section and variable casing");
    assert_eq!(entry.value, "second");
    assert_eq!(entry.scope, GitConfigScope::Command);
}
