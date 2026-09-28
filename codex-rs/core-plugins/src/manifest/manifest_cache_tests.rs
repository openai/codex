//! Verify parse reuse, revision invalidation, and diagnostic preservation.

use super::super::parse_legacy_plugin_manifest_uri;
use super::super::parse_resolved_plugin_manifest;
use super::ManifestCache;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::cell::Cell;
use std::sync::Mutex;
use tracing_test::internal::MockWriter;

#[test]
fn repeated_revision_runs_parser_once_and_changes_run_it_again() {
    let directory = tempfile::tempdir().unwrap();
    let root = PathUri::from_host_native_path(directory.path()).unwrap();
    let path = root.join(".codex-plugin/plugin.json").unwrap();
    let first = r#"{"name":"demo","description":"first"}"#;
    let changed = r#"{"name":"demo","description":"changed"}"#;
    let parses = Cell::new(0);
    let cache = ManifestCache::default();
    let mut results = Vec::new();
    for contents in std::iter::repeat_n(first, 220).chain([changed, first]) {
        results.push(
            cache
                .parse(&root, &path, contents, /*overlay*/ None, || {
                    parses.set(parses.get() + 1);
                    parse_legacy_plugin_manifest_uri(&root, &path, contents)
                })
                .unwrap(),
        );
    }
    let original = results[0].clone();
    let mut revised = original.clone();
    revised.description = Some("changed".to_string());
    assert_eq!(
        results,
        std::iter::repeat_n(original.clone(), 220)
            .chain([revised, original])
            .collect::<Vec<_>>()
    );
    assert_eq!(parses.get(), 3);
}

#[test]
fn invalid_fields_warn_once_until_the_manifest_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join(".codex-plugin/plugin.json");
    let buffer: &'static Mutex<Vec<u8>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(/*ansi*/ false)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let original =
        r#"{"name":"demo","interface":{"defaultPrompt":" ","composerIcon":"../outside.svg"}}"#;
    let changed = r#"{"name":"demo","description":"changed","interface":{"defaultPrompt":" ","composerIcon":"../outside.svg"}}"#;
    let cache = ManifestCache::default();
    let parse = |contents| {
        parse_resolved_plugin_manifest(
            &cache,
            directory.path(),
            &path,
            contents,
            /*overlay*/ None,
        )
        .unwrap()
    };
    let first = parse(original);
    for _ in 0..220 {
        assert_eq!(parse(original), first);
    }
    let mut expected = first.clone();
    expected.description = Some("changed".to_string());
    assert_eq!(parse(changed), expected);
    assert_eq!(parse(original), first);
    let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert_eq!(logs.matches("ignoring interface.defaultPrompt").count(), 3);
    assert_eq!(logs.matches("ignoring interface.composerIcon").count(), 3);
}

#[test]
fn overlay_revision_and_resource_root_are_part_of_cached_parse() {
    let directory = tempfile::tempdir().unwrap();
    let root = PathUri::from_host_native_path(directory.path()).unwrap();
    let other_root = root.join("other").unwrap();
    let path = root.join("plugin.json").unwrap();
    let overlay_path = root.join(".codex-plugin/plugin.json").unwrap();
    let contents = serde_json::json!({
        "$schema": codex_utils_plugins::AGENT_PLUGIN_SCHEMA_URI,
        "name": "demo"
    })
    .to_string();
    let first = r#"{"interface":{"defaultPrompt":"first"}}"#;
    let changed = r#"{"interface":{"defaultPrompt":"changed"}}"#;
    let parses = Cell::new(0);
    let cache = ManifestCache::default();
    for (plugin_root, overlay) in [
        (&root, Some((&overlay_path, first))),
        (&root, Some((&overlay_path, first))),
        (&root, Some((&overlay_path, changed))),
        (&root, None),
        (&other_root, None),
    ] {
        let manifest = cache
            .parse(plugin_root, &path, &contents, overlay, || {
                parses.set(parses.get() + 1);
                super::super::agent_plugin_manifest::parse_agent_plugin_manifest_uri(
                    plugin_root,
                    &path,
                    &contents,
                    overlay,
                )
            })
            .unwrap();
        let expected = super::super::agent_plugin_manifest::parse_agent_plugin_manifest_uri(
            plugin_root,
            &path,
            &contents,
            overlay,
        )
        .unwrap();
        assert_eq!(manifest, expected);
    }
    assert_eq!(parses.get(), 4);
}

#[test]
fn failed_revision_is_retried_and_repair_is_visible() {
    let directory = tempfile::tempdir().unwrap();
    let root = PathUri::from_host_native_path(directory.path()).unwrap();
    let path = root.join(".codex-plugin/plugin.json").unwrap();
    let cache = ManifestCache::default();
    for contents in ["{", "{"] {
        assert!(
            cache
                .parse(&root, &path, contents, /*overlay*/ None, || {
                    parse_legacy_plugin_manifest_uri(&root, &path, contents)
                })
                .is_err()
        );
    }
    let repaired = r#"{"name":"repaired"}"#;
    assert_eq!(
        cache
            .parse(&root, &path, repaired, /*overlay*/ None, || {
                parse_legacy_plugin_manifest_uri(&root, &path, repaired)
            })
            .unwrap(),
        parse_legacy_plugin_manifest_uri(&root, &path, repaired).unwrap()
    );
}

#[test]
fn equivalent_windows_paths_share_a_cached_revision() {
    let cache = ManifestCache::default();
    let parses = Cell::new(0);
    let contents = r#"{"name":"demo"}"#;
    let mut manifests = Vec::new();
    for value in ["file:///C:/plugins/Demo", "file:///C:/plugins/demo"] {
        let root = PathUri::parse(value).unwrap();
        let path = root.join(".codex-plugin/plugin.json").unwrap();
        manifests.push(
            cache
                .parse(&root, &path, contents, /*overlay*/ None, || {
                    parses.set(parses.get() + 1);
                    parse_legacy_plugin_manifest_uri(&root, &path, contents)
                })
                .unwrap(),
        );
    }
    assert_eq!(manifests[0], manifests[1]);
    assert_eq!(parses.get(), 1);
}

#[test]
fn oversized_revision_invalidates_previous_result_without_being_cached() {
    let directory = tempfile::tempdir().unwrap();
    let root = PathUri::from_host_native_path(directory.path()).unwrap();
    let path = root.join(".codex-plugin/plugin.json").unwrap();
    let small = r#"{"name":"demo"}"#;
    let large = format!("{}{small}", " ".repeat(super::MAX_CACHABLE_INPUT_LEN));
    let parses = Cell::new(0);
    let cache = ManifestCache::default();
    for contents in [small, small, &large, &large, small] {
        cache
            .parse(&root, &path, contents, /*overlay*/ None, || {
                parses.set(parses.get() + 1);
                parse_legacy_plugin_manifest_uri(&root, &path, contents)
            })
            .unwrap();
    }
    assert_eq!(parses.get(), 4);
}

#[test]
fn oversized_parse_does_not_block_unrelated_manifest() {
    use std::sync::mpsc;
    use std::time::Duration;

    let directory = tempfile::tempdir().unwrap();
    let root = PathUri::from_host_native_path(directory.path()).unwrap();
    let large_path = root.join("large.json").unwrap();
    let small_path = root.join("small.json").unwrap();
    let large = format!("{}{{}}", " ".repeat(super::MAX_CACHABLE_INPUT_LEN));
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (completed_tx, completed_rx) = mpsc::channel();
    let root = &root;
    let large_path = &large_path;
    let small_path = &small_path;
    let large = &large;
    let cache = &ManifestCache::default();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            cache
                .parse(root, large_path, large, /*overlay*/ None, || {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    parse_legacy_plugin_manifest_uri(root, large_path, large)
                })
                .unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        scope.spawn(move || {
            cache
                .parse(root, small_path, "{}", /*overlay*/ None, || {
                    parse_legacy_plugin_manifest_uri(root, small_path, "{}")
                })
                .unwrap();
            completed_tx.send(()).unwrap();
        });
        let completed = completed_rx.recv_timeout(Duration::from_secs(10));
        release_tx.send(()).unwrap();
        assert_eq!(completed, Ok(()));
    });
}

#[test]
fn store_clones_reuse_validation_but_independent_stores_warn_again() {
    use crate::store::PluginStore;

    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugin");
    std::fs::create_dir_all(root.join(".codex-plugin")).unwrap();
    std::fs::write(
        root.join(".codex-plugin/plugin.json"),
        r#"{"name":"demo","interface":{"defaultPrompt":" "}}"#,
    )
    .unwrap();
    let buffer: &'static Mutex<Vec<u8>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(/*ansi*/ false)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let store = PluginStore::new(directory.path().to_path_buf());
    let cloned_store = store.clone();
    let first = store.manifest_cache.load(&root).unwrap().manifest;
    for _ in 0..220 {
        assert_eq!(
            cloned_store.manifest_cache.load(&root).unwrap().manifest,
            first
        );
    }
    let independent_store = PluginStore::new(directory.path().to_path_buf());
    assert_eq!(
        independent_store
            .manifest_cache
            .load(&root)
            .unwrap()
            .manifest,
        first
    );
    let logs = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert_eq!(
        logs.matches("ignoring interface.defaultPrompt: prompt must not be empty")
            .count(),
        2
    );
}

#[test]
fn disabled_cache_parses_each_load() {
    let directory = tempfile::tempdir().unwrap();
    let root = PathUri::from_host_native_path(directory.path()).unwrap();
    let path = root.join(".codex-plugin/plugin.json").unwrap();
    let contents = r#"{"name":"demo"}"#;
    let expected = parse_legacy_plugin_manifest_uri(&root, &path, contents).unwrap();
    let cache = ManifestCache::disabled();
    let parses = Cell::new(0);
    for _ in 0..3 {
        let result = cache
            .parse(&root, &path, contents, /*overlay*/ None, || {
                parses.set(parses.get() + 1);
                parse_legacy_plugin_manifest_uri(&root, &path, contents)
            })
            .unwrap();
        assert_eq!(result, expected);
    }
    assert_eq!(parses.get(), 3);
}
