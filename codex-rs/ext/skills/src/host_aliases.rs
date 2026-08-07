use std::collections::HashMap;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use crate::catalog::SkillCatalogEntry;

pub(crate) fn shared_host_alias_roots(entries: &[&SkillCatalogEntry]) -> Vec<String> {
    let mut plugin_version_skill_counts = HashMap::new();
    for root in entries.iter().filter_map(|entry| entry.alias_root()) {
        let Some(plugin_version) = plugin_version_base(Path::new(root)) else {
            continue;
        };
        let count = plugin_version_skill_counts
            .entry(plugin_version)
            .or_insert(0usize);
        *count = count.saturating_add(1);
    }

    entries
        .iter()
        .filter_map(|entry| entry.alias_root())
        .map(|root| {
            let root = Path::new(root);
            let shared_root = match plugin_version_base(root) {
                Some(plugin_version)
                    if plugin_version_skill_counts
                        .get(&plugin_version)
                        .copied()
                        .unwrap_or_default()
                        <= 1 =>
                {
                    plugin_marketplace_base(root).unwrap_or_else(|| root.to_path_buf())
                }
                Some(_) | None => root.to_path_buf(),
            };

            shared_root.to_string_lossy().replace('\\', "/")
        })
        .collect()
}

fn plugin_marketplace_base(path: &Path) -> Option<PathBuf> {
    let mut candidate = path;
    while let Some(parent) = candidate.parent() {
        if parent.file_name()?.to_str()? == "cache"
            && parent.parent()?.file_name()?.to_str()? == "plugins"
        {
            return Some(candidate.to_path_buf());
        }
        candidate = parent;
    }
    None
}

fn plugin_version_base(path: &Path) -> Option<PathBuf> {
    let marketplace_base = plugin_marketplace_base(path)?;
    let mut relative_components = path.strip_prefix(&marketplace_base).ok()?.components();
    let plugin = match relative_components.next()? {
        Component::Normal(plugin) => plugin,
        _ => return None,
    };
    let version = match relative_components.next()? {
        Component::Normal(version) => version,
        _ => return None,
    };
    Some(marketplace_base.join(plugin).join(version))
}
