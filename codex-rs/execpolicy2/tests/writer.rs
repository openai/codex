use codex_execpolicy2::Decision;
use codex_execpolicy2::append_prefix_rule;
use std::fs;
use tempfile::tempdir;

#[test]
fn appends_creating_missing_policy_file() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("policy").join("default.codexpolicy");

    append_prefix_rule(
        &path,
        &["rg".to_string(), "--files".to_string()],
        Decision::Allow,
    )
    .expect("append rule");

    let contents = fs::read_to_string(path).expect("read policy");
    assert_eq!(
        contents,
        "prefix_rule(pattern=[\"rg\", \"--files\"], decision=\"allow\")\n"
    );
}

#[test]
fn appends_rules_with_newlines() {
    let dir = tempdir().expect("create temp dir");
    let path = dir.path().join("policy.codexpolicy");
    append_prefix_rule(&path, &["ls".to_string()], Decision::Allow).expect("append first rule");
    append_prefix_rule(&path, &["pwd".to_string()], Decision::Allow).expect("append second rule");

    let contents = fs::read_to_string(path).expect("read policy");
    assert_eq!(
        contents,
        "\
prefix_rule(pattern=[\"ls\"], decision=\"allow\")
prefix_rule(pattern=[\"pwd\"], decision=\"allow\")
"
    );
}
