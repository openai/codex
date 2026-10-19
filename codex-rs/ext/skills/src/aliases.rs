//! Bounded, append-only aliases shared by rendering and reads within a live thread.
//! Step snapshots retain their assignments; aliases are not persisted across resume or fork.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use serde::Deserialize;
use serde::Serialize;

use crate::catalog::SkillSourceKind;
use crate::step_state::SkillToolAuthoritySelector;

fn alias_prefix(source: &SkillSourceKind) -> Option<&'static str> {
    match source {
        SkillSourceKind::Cloud => Some("c"),
        SkillSourceKind::Executor => Some("e"),
        SkillSourceKind::Host => Some("r"),
        SkillSourceKind::Custom(_) => None,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(from = "String", into = "String")]
pub(crate) struct SkillLocator {
    value: String,
    alias_source: Option<SkillSourceKind>,
}

impl From<String> for SkillLocator {
    fn from(value: String) -> Self {
        let alias_source = value.split_once('/').and_then(|(root, suffix)| {
            if suffix.is_empty() {
                return None;
            }
            let (prefix, index) = root.split_at_checked(1)?;
            if index.is_empty()
                || !index.bytes().all(|byte| byte.is_ascii_digit())
                || (index.len() > 1 && index.starts_with('0'))
            {
                return None;
            }
            index.parse::<usize>().ok()?;
            [
                SkillSourceKind::Cloud,
                SkillSourceKind::Executor,
                SkillSourceKind::Host,
            ]
            .into_iter()
            .find(|source| alias_prefix(source) == Some(prefix))
        });
        Self {
            value,
            alias_source,
        }
    }
}

impl From<SkillLocator> for String {
    fn from(locator: SkillLocator) -> Self {
        locator.value
    }
}

impl SkillLocator {
    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) fn alias_source(&self) -> Option<&SkillSourceKind> {
        self.alias_source.as_ref()
    }
}

/// Owns thread-wide alias allocation independently of provider caches.
#[derive(Default)]
pub(crate) struct SkillAliasState {
    aliases: Mutex<SkillCatalogAliases>,
}

impl SkillAliasState {
    pub(crate) fn capture(
        &self,
        roots: impl IntoIterator<Item = (SkillSourceKind, String)>,
    ) -> SkillCatalogAliases {
        let mut aliases = self
            .aliases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (kind, root) in roots {
            let selector = match kind {
                SkillSourceKind::Cloud => SkillToolAuthoritySelector::Cloud,
                SkillSourceKind::Executor => SkillToolAuthoritySelector::Executor,
                SkillSourceKind::Host | SkillSourceKind::Custom(_) => continue,
            };
            aliases.extend(selector, [root.as_str()]);
        }
        aliases.clone()
    }
}

/// Thread-wide assignments, captured alongside each step's catalog.
#[derive(Clone, Debug, Default)]
pub(crate) struct SkillCatalogAliases {
    cloud: Arc<AliasPlan>,
    executor: Arc<AliasPlan>,
    budget: AliasBudget,
}

impl SkillCatalogAliases {
    pub(crate) fn resolve(&self, locator: &SkillLocator) -> Option<String> {
        let plan = match locator.alias_source() {
            Some(SkillSourceKind::Cloud) => &self.cloud,
            Some(SkillSourceKind::Executor) => &self.executor,
            Some(SkillSourceKind::Host | SkillSourceKind::Custom(_)) | None => return None,
        };
        plan.expand(locator.as_str())
    }

    fn extend<'a>(
        &mut self,
        selector: SkillToolAuthoritySelector,
        roots: impl IntoIterator<Item = &'a str>,
    ) {
        let (plan, prefix) = match selector {
            SkillToolAuthoritySelector::Cloud => (&mut self.cloud, "c"),
            SkillToolAuthoritySelector::Executor => (&mut self.executor, "e"),
        };
        plan.extend_alias_plan(prefix, roots, &mut self.budget);
    }

    pub(crate) fn for_authority(&self, selector: SkillToolAuthoritySelector) -> &AliasPlan {
        match selector {
            SkillToolAuthoritySelector::Cloud => &self.cloud,
            SkillToolAuthoritySelector::Executor => &self.executor,
        }
    }
}

const MAX_ALIAS_ROOTS: usize = 2_048;
const MAX_ALIAS_BYTES: usize = 16 * 1024;

/// Shared bound on thread-local Cloud/executor alias assignments and their rendered mappings.
#[derive(Clone, Debug, Default)]
struct AliasBudget {
    roots: usize,
    bytes: usize,
}

#[derive(Clone, Debug)]
struct AliasRoot {
    name: String,
    value: Arc<str>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AliasPlan {
    roots: Vec<AliasRoot>,
    known_roots: HashSet<Arc<str>>,
}

impl AliasPlan {
    pub(crate) fn build(source: &SkillSourceKind, candidates: &[&str]) -> Option<Self> {
        let prefix = alias_prefix(source)?;
        let mut roots = Vec::new();
        let mut known_roots = HashSet::new();

        for &root in candidates {
            if known_roots.contains(root) {
                continue;
            }
            let root: Arc<str> = root.into();
            known_roots.insert(Arc::clone(&root));

            let index = roots.len();
            roots.push(AliasRoot {
                name: format!("{prefix}{index}"),
                value: root,
            });
        }

        (!roots.is_empty()).then_some(Self { roots, known_roots })
    }

    /// Retains existing assignments and reserves new numbers for previously unseen roots.
    fn extend_alias_plan<'a>(
        self: &mut Arc<Self>,
        prefix: &str,
        roots: impl IntoIterator<Item = &'a str>,
        budget: &mut AliasBudget,
    ) {
        for root in roots {
            if self.known_roots.contains(root) {
                continue;
            }
            let name = format!("{prefix}{}", self.roots.len());
            let bytes = name.len() + root.len() + "- `` = ``".len();
            if budget.roots == MAX_ALIAS_ROOTS {
                break;
            }
            if bytes > MAX_ALIAS_BYTES - budget.bytes {
                continue;
            }
            budget.roots += 1;
            budget.bytes += bytes;
            let plan = Arc::make_mut(self);
            let root: Arc<str> = root.into();
            plan.known_roots.insert(Arc::clone(&root));
            plan.roots.push(AliasRoot { name, value: root });
        }
    }

    pub(crate) fn shorten(&self, locator: &str) -> Option<String> {
        self.roots
            .iter()
            .filter_map(|root| {
                let suffix = locator
                    .strip_prefix(root.value.trim_end_matches('/'))?
                    .strip_prefix('/')?;
                Some((root, suffix))
            })
            .max_by_key(|(root, _)| root.value.len())
            .map(|(root, suffix)| format!("{}/{suffix}", root.name))
    }

    fn expand(&self, alias: &str) -> Option<String> {
        let (name, suffix) = alias.split_once('/')?;
        let root = self.roots.iter().find(|root| root.name == name)?;
        Some(format!("{}/{suffix}", root.value.trim_end_matches('/')))
    }

    pub(crate) fn root_lines_for(&self, used: &HashSet<String>) -> Vec<String> {
        self.roots
            .iter()
            .filter(|root| used.contains(&root.name))
            .map(|root| format!("- `{}` = `{}`", root.name, root.value))
            .collect()
    }

    pub(crate) fn root_lines(&self) -> Vec<String> {
        self.roots
            .iter()
            .map(|root| format!("- `{}` = `{}`", root.name, root.value))
            .collect()
    }
}

#[cfg(test)]
#[path = "aliases_tests.rs"]
mod tests;
