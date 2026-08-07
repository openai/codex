use std::collections::HashSet;

struct AliasRoot {
    name: String,
    value: String,
}

pub(crate) struct AliasPlan {
    roots: Vec<AliasRoot>,
}

impl AliasPlan {
    pub(crate) fn build(prefix: &str, candidates: &[&str]) -> Option<Self> {
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
