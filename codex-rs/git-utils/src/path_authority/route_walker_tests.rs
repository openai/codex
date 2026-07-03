use pretty_assertions::assert_eq;

use super::read_link_if_symlink;

#[test]
fn ordinary_paths_are_not_treated_as_failed_symlink_reads() {
    let fixture = tempfile::tempdir().expect("fixture");
    let directory = fixture.path().join("directory");
    let file = fixture.path().join("file");
    std::fs::create_dir(&directory).expect("directory");
    std::fs::write(&file, b"ordinary file").expect("file");

    assert_eq!(
        read_link_if_symlink(&directory).expect("ordinary directory"),
        None
    );
    assert_eq!(read_link_if_symlink(&file).expect("ordinary file"), None);
}
