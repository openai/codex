use std::collections::BTreeSet;

/// Opaque host features available to runtime plugin contributions.
///
/// Capability names are case-sensitive. Construction trims surrounding
/// whitespace, drops empty names, and deduplicates values deterministically.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct HostCapabilities(BTreeSet<String>);

impl HostCapabilities {
    pub fn from_names<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self(
            names
                .into_iter()
                .filter_map(|name| {
                    let name = name.as_ref().trim();
                    (!name.is_empty()).then(|| name.to_string())
                })
                .collect(),
        )
    }

    pub fn contains(&self, capability: &str) -> bool {
        self.0.contains(capability)
    }

    pub fn insert(&mut self, capability: impl AsRef<str>) -> bool {
        let capability = capability.as_ref().trim();
        !capability.is_empty() && self.0.insert(capability.to_string())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

#[cfg(test)]
#[path = "host_capabilities_tests.rs"]
mod tests;
