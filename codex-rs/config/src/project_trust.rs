//! Match merged project settings against ordered native path spellings.

use crate::config_toml::ConfigToml;
use crate::config_toml::ProjectConfig;
use codex_utils_path::normalize_for_path_comparison;
use codex_utils_path_uri::PathConvention;
use std::path::Path;

/// Ordered project lookup keys for one executor working directory.
///
/// Resolve this independently of configuration layers, then apply it after all
/// local and cloud projects maps have been merged. Cwd entries, including an
/// entry without a trust level, take precedence over repository-root entries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTrustLookup {
    convention: PathConvention,
    keys: Vec<String>,
}

impl ProjectTrustLookup {
    pub(crate) fn from_native(cwd: &Path, repo_root: Option<&Path>) -> Self {
        Self::from_paths(
            PathConvention::native(),
            std::iter::once(cwd).chain(repo_root).map(|path| {
                (
                    path.to_string_lossy().into_owned(),
                    normalize_for_path_comparison(path)
                        .ok()
                        .map(|path| path.to_string_lossy().into_owned()),
                )
            }),
        )
    }

    /// Build lookup keys from `(original, canonical)` native path spellings.
    ///
    /// Supply the cwd first, followed by the optional repository root. Each
    /// canonical spelling is tried before its original spelling; `None` uses
    /// only the original. This method performs no filesystem access or path
    /// resolution. Callers must supply executor-normalized canonical spellings,
    /// including Windows extended-length prefixes and WSL mounted-drive folding
    /// where applicable. Original spellings remain literal. Windows keys are
    /// matched without ASCII case distinctions; POSIX keys remain case-sensitive.
    pub fn from_paths(
        convention: PathConvention,
        paths: impl IntoIterator<Item = (String, Option<String>)>,
    ) -> Self {
        let mut keys = Vec::new();
        for (original, canonical) in paths {
            let original = normalize_lookup_key(&original, convention);
            let canonical = canonical
                .map(|key| normalize_lookup_key(&key, convention))
                .unwrap_or_else(|| original.clone());
            if canonical != original {
                keys.push(canonical);
            }
            keys.push(original);
        }
        Self { convention, keys }
    }
}

impl ConfigToml {
    /// Select the active project from the final merged projects map.
    pub fn get_active_project_for_lookup(
        &self,
        lookup: &ProjectTrustLookup,
    ) -> Option<ProjectConfig> {
        let projects = self.projects.as_ref()?;
        for key in &lookup.keys {
            if let Some(project) = projects.get(key) {
                return Some(project.clone());
            }
            // Preserve native exact-key preference and deterministic selection
            // when multiple Windows keys differ only in ASCII case.
            if let Some((_, project)) = projects
                .iter()
                .filter(|(candidate, _)| normalize_lookup_key(candidate, lookup.convention) == *key)
                .min_by_key(|(candidate, _)| *candidate)
            {
                return Some(project.clone());
            }
        }
        None
    }
}

fn normalize_lookup_key(key: &str, convention: PathConvention) -> String {
    match convention {
        PathConvention::Windows => key.to_ascii_lowercase(),
        PathConvention::Posix => key.to_owned(),
    }
}

#[cfg(test)]
#[path = "project_trust_tests.rs"]
mod tests;
