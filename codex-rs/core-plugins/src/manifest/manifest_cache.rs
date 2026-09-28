//! Reuse parsed manifest revisions within an explicitly owned plugin store.
//! Store clones share the cache; independently created stores do not share entries or warnings.

use super::LoadedPluginManifest;
use super::PluginManifestFormat;
use super::UriPluginManifest;
use super::parse_resolved_plugin_manifest;
use codex_utils_path_uri::PathUri;
use codex_utils_plugins::AGENT_PLUGIN_MANIFEST_RELATIVE_PATH;
use codex_utils_plugins::find_plugin_manifest_path;
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

const CAPACITY: usize = 128;
const MAX_CACHABLE_INPUT_LEN: usize = 64 * 1024;

pub(crate) struct ManifestCache {
    state: Option<Mutex<ManifestCacheState>>,
}

#[derive(Default)]
struct ManifestCacheState {
    entries: HashMap<(PathUri, PathUri), Entry>,
    recency: VecDeque<(PathUri, PathUri)>,
}

struct Entry {
    contents_digest: [u8; 32],
    overlay: Option<(PathUri, [u8; 32])>,
    manifest: UriPluginManifest,
}

impl std::fmt::Debug for ManifestCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManifestCache")
            .finish_non_exhaustive()
    }
}

impl Default for ManifestCache {
    fn default() -> Self {
        Self {
            state: Some(Mutex::default()),
        }
    }
}

impl ManifestCache {
    pub(crate) fn disabled() -> Self {
        Self { state: None }
    }

    pub(crate) fn load(&self, plugin_root: &Path) -> Option<LoadedPluginManifest> {
        let manifest_path = find_plugin_manifest_path(plugin_root)?;
        let contents = fs::read_to_string(&manifest_path).ok()?;
        let is_agent_plugin =
            manifest_path == plugin_root.join(AGENT_PLUGIN_MANIFEST_RELATIVE_PATH);
        let overlay = if is_agent_plugin {
            let overlay_path = plugin_root.join(".codex-plugin/plugin.json");
            fs::read_to_string(&overlay_path)
                .ok()
                .map(|contents| (overlay_path, contents))
        } else {
            None
        };
        match parse_resolved_plugin_manifest(
            self,
            plugin_root,
            &manifest_path,
            &contents,
            overlay
                .as_ref()
                .map(|(path, contents)| (path.as_path(), contents.as_str())),
        ) {
            Ok(manifest) => Some(LoadedPluginManifest {
                manifest,
                format: if is_agent_plugin {
                    PluginManifestFormat::AgentPlugin
                } else {
                    PluginManifestFormat::Legacy
                },
            }),
            Err(err) => {
                tracing::warn!(
                    path = %manifest_path.display(),
                    "failed to parse plugin manifest: {err}"
                );
                None
            }
        }
    }

    pub(crate) fn parse(
        &self,
        root: &PathUri,
        path: &PathUri,
        contents: &str,
        overlay: Option<(&PathUri, &str)>,
        parse: impl FnOnce() -> Result<UriPluginManifest, serde_json::Error>,
    ) -> Result<UriPluginManifest, serde_json::Error> {
        let Some(state) = &self.state else {
            return parse();
        };
        if contents
            .len()
            .saturating_add(overlay.map_or(0, |(_, value)| value.len()))
            > MAX_CACHABLE_INPUT_LEN
        {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&(root.clone(), path.clone()));
            // Unbounded inputs must not serialize unrelated parses behind the cache lock.
            return parse();
        }
        // Parsing is synchronous and does no filesystem I/O. Serializing cache misses also
        // prevents concurrent capability loaders from validating the same revision twice.
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .parse(root, path, contents, overlay, parse)
    }
}

impl ManifestCacheState {
    fn remove(&mut self, key: &(PathUri, PathUri)) -> Option<Entry> {
        let entry = self.entries.remove(key)?;
        self.recency.retain(|existing| existing != key);
        Some(entry)
    }

    fn parse(
        &mut self,
        root: &PathUri,
        path: &PathUri,
        contents: &str,
        overlay: Option<(&PathUri, &str)>,
        parse: impl FnOnce() -> Result<UriPluginManifest, serde_json::Error>,
    ) -> Result<UriPluginManifest, serde_json::Error> {
        let key = (root.clone(), path.clone());
        let previous = self.remove(&key);
        let contents_digest = Sha256::digest(contents.as_bytes()).into();
        let overlay = overlay
            .map(|(path, contents)| (path.clone(), Sha256::digest(contents.as_bytes()).into()));
        if let Some(entry) = previous
            && entry.contents_digest == contents_digest
            && entry.overlay == overlay
        {
            let manifest = entry.manifest.clone();
            self.recency.push_front(key.clone());
            self.entries.insert(key, entry);
            return Ok(manifest);
        }
        let manifest = parse()?;
        if self.entries.len() == CAPACITY
            && let Some(oldest) = self.recency.pop_back()
        {
            self.entries.remove(&oldest);
        }
        self.recency.push_front(key.clone());
        self.entries.insert(
            key,
            Entry {
                contents_digest,
                overlay,
                manifest: manifest.clone(),
            },
        );
        Ok(manifest)
    }
}

#[cfg(test)]
#[path = "manifest_cache_tests.rs"]
mod tests;
