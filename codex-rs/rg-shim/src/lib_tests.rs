use super::InventoryCache;
use super::find_repository_root;
use super::open_file_inventory;
use pretty_assertions::assert_eq;
use std::fs;
use std::io::Read;
use tempfile::TempDir;

#[test]
fn finds_nearest_git_worktree_root() {
    let temp_dir = TempDir::new().expect("temp dir");
    let outer = temp_dir.path().join("outer");
    let inner = outer.join("nested");
    let child = inner.join("src");
    fs::create_dir_all(outer.join(".git")).expect("outer git dir");
    fs::create_dir_all(inner.join(".git")).expect("inner git dir");
    fs::create_dir_all(&child).expect("child dir");

    assert_eq!(
        find_repository_root(&child),
        Some(fs::canonicalize(inner).expect("canonical inner root"))
    );
}

#[test]
fn opens_only_matching_ready_inventory_at_repository_root() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repository = temp_dir.path().join("repository");
    let child = repository.join("src");
    let cache_root = temp_dir.path().join("cache");
    fs::create_dir_all(repository.join(".git")).expect("git dir");
    fs::create_dir_all(&child).expect("child dir");
    let repository = fs::canonicalize(repository).expect("canonical repository");
    let cache = InventoryCache::new(&cache_root, &repository).expect("cache layout");
    fs::create_dir_all(&cache.directory).expect("cache dir");
    fs::write(&cache.root, repository.to_string_lossy().as_bytes()).expect("root marker");
    fs::write(&cache.files, "src/lib.rs\n").expect("inventory");

    assert!(open_file_inventory(&cache_root, &repository).is_none());
    fs::write(&cache.ready, "generation-1").expect("ready marker");
    assert!(open_file_inventory(&cache_root, &child).is_none());

    let mut inventory = String::new();
    open_file_inventory(&cache_root, &repository)
        .expect("ready inventory")
        .read_to_string(&mut inventory)
        .expect("read inventory");
    assert_eq!(inventory, "src/lib.rs\n");

    fs::write(&cache.root, "/different/repository").expect("mismatched root marker");
    assert!(open_file_inventory(&cache_root, &repository).is_none());
}
