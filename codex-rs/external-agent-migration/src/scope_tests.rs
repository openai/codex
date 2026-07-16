use super::MigrationScope;
use pretty_assertions::assert_eq;

#[test]
fn missing_cwd_selects_home_scope() {
    assert_eq!(
        MigrationScope::from_cwd(/*cwd*/ None).expect("resolve scope"),
        Some(MigrationScope::Home)
    );
}

#[test]
fn nested_cwd_selects_repository_root() {
    let root = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir(root.path().join(".git")).expect("create git directory");
    let nested = root.path().join("src").join("nested");
    std::fs::create_dir_all(&nested).expect("create nested directory");

    assert_eq!(
        MigrationScope::from_cwd(Some(&nested)).expect("resolve scope"),
        Some(MigrationScope::Repository {
            root: root.path().to_path_buf(),
        })
    );
}

#[test]
fn nonexistent_cwd_has_no_scope() {
    let root = tempfile::tempdir().expect("tempdir");

    assert_eq!(
        MigrationScope::from_cwd(Some(&root.path().join("missing"))).expect("resolve scope"),
        None
    );
}
