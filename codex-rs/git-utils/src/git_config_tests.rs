use super::*;

#[test]
fn path_containment_uses_component_boundaries() {
    let root = Path::new("/repo/root");
    assert!(path_is_within(Path::new("/repo/root"), root));
    assert!(path_is_within(Path::new("/repo/root/config"), root));
    assert!(!path_is_within(Path::new("/repo/rooted/config"), root));
    assert!(!path_is_within(Path::new("/repo"), root));
}
