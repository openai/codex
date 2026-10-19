use std::collections::HashSet;

use serde::Deserialize;
use serde::Serialize;

use crate::catalog::SkillSourceKind;

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

struct AliasRoot {
    name: String,
    value: String,
}

pub(crate) struct AliasPlan {
    roots: Vec<AliasRoot>,
}

impl AliasPlan {
    pub(crate) fn build(source: &SkillSourceKind, candidates: &[&str]) -> Option<Self> {
        let prefix = alias_prefix(source)?;
        let mut roots = Vec::new();
        let mut seen = HashSet::new();

        for &root in candidates {
            if !seen.insert(root) {
                continue;
            }

            let index = roots.len();
            roots.push(AliasRoot {
                name: format!("{prefix}{index}"),
                value: root.to_string(),
            });
        }

        (!roots.is_empty()).then_some(Self { roots })
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
