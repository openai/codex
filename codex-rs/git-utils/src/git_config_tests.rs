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
        assert!(parse_effective_config(output).is_err(), "{output:?}");
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
