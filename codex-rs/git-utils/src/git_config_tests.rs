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
            origin: "file:/repo/.git/config".to_string(),
            key: "filter.demo.clean".to_string(),
            value: String::new(),
        })
    );
    assert_eq!(
        entries.get("merge.Name.driver"),
        Some(&GitConfigEntry {
            scope: GitConfigScope::Command,
            origin: "command line:".to_string(),
            key: "merge.Name.driver".to_string(),
            value: "helper %A %B".to_string(),
        })
    );
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
            origin: "file:/repo/.git/config".to_string(),
            key: "filter.demo.clean".to_string(),
            value: String::new(),
        })
    );
    assert_eq!(
        entries.get("merge.Name.driver"),
        Some(&GitConfigEntry {
            scope: GitConfigScope::Command,
            origin: "command line:".to_string(),
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
