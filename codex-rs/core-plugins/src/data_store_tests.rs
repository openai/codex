use super::*;
use codex_plugin::PluginId;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn local_plugin_data_root_derives_path_from_key() {
    let tmp = tempdir().unwrap();
    let store = LocalPluginDataStore::from_codex_home(tmp.path().to_path_buf()).unwrap();
    let plugin_id = PluginId::new("sample".to_string(), "debug".to_string()).unwrap();

    assert_eq!(
        store.plugin_data_root(&plugin_id).as_path(),
        tmp.path().join("plugins/data/sample-debug")
    );
}
