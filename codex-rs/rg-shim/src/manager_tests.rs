use super::build_inventory;
use super::invalidate;
use super::publish;
use crate::InventoryCache;
use pretty_assertions::assert_eq;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

#[test]
fn built_inventory_is_visible_only_while_ready() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repository = temp_dir.path().join("repository");
    let cache_root = temp_dir.path().join("cache");
    fs::create_dir_all(repository.join(".git")).expect("git dir");
    fs::create_dir_all(&cache_root).expect("cache root");
    let repository = fs::canonicalize(repository).expect("canonical repository");
    let cache = InventoryCache::new(&cache_root, &repository).expect("cache layout");
    fs::create_dir_all(&cache.directory).expect("cache dir");
    fs::write(&cache.root, repository.to_string_lossy().as_bytes()).expect("root marker");

    let fake_rg = temp_dir.path().join("rg");
    fs::write(&fake_rg, "#!/bin/sh\nprintf 'src/lib.rs\\nREADME.md\\n'\n").expect("fake rg");
    let mut permissions = fs::metadata(&fake_rg)
        .expect("fake rg metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_rg, permissions).expect("fake rg permissions");

    let inventory =
        build_inventory(&fake_rg, &repository, &cache.directory).expect("build inventory");
    assert!(cache.open().is_none());

    publish(&cache, inventory).expect("publish inventory");
    let mut output = String::new();
    cache
        .open()
        .expect("ready inventory")
        .read_to_string(&mut output)
        .expect("read inventory");
    assert_eq!(output, "src/lib.rs\nREADME.md\n");

    invalidate(&cache);
    assert!(cache.open().is_none());
}
