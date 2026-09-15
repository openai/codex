//! Verifies Windows selection without requiring an elevated daemon.
use pretty_assertions::assert_eq;

#[cfg(windows)]
#[test]
fn daemon_junction_can_be_created_and_retargeted_without_cli_links() {
    let home = tempfile::TempDir::new().unwrap();
    let root = home.path().join("packages/app-server-daemon");
    for version in ["first", "second"] {
        let release = root.join("releases").join(version);
        std::fs::create_dir_all(&release).unwrap();
        super::select_release(&root, &release).unwrap();
        assert_eq!(
            root.join("current").canonicalize().unwrap(),
            release.canonicalize().unwrap()
        );
    }
    assert!(!home.path().join("packages/standalone").exists());
}
