//! Bounded rendering of host-verified invoked skill paths for async review.
//! Ownership checks, turn storage and delegated evidence selection stay in the host.

use crate::ContextSection;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionInput;
use crate::SectionScope;
use codex_context_fragments::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use codex_protocol::protocol::TruncationPolicy;
use serde_json::json;

const MAX_TRUSTED_SKILL_TOKENS: usize = 768;
const TRUSTED_SKILLS_PREFIX: &str = "Codex-verified invoked user-owned skill paths:\n";

/// Host-verified user-owned skill paths for the current Guardian classification.
#[derive(Clone, PartialEq, Eq)]
pub struct TrustedSkills {
    pub paths: Vec<String>,
}

impl ContextualUserFragment for TrustedSkills {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.trusted_skills".to_owned())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("", "")
    }

    fn body(&self) -> String {
        let mut paths = String::from("[");
        let max_path_bytes = TruncationPolicy::Tokens(MAX_TRUSTED_SKILL_TOKENS)
            .byte_budget()
            .saturating_sub(TRUSTED_SKILLS_PREFIX.len());
        for path in &self.paths {
            let separator = if paths.ends_with('[') { "" } else { "," };
            let encoded_path = json!(path).to_string();
            if paths
                .len()
                .saturating_add(separator.len())
                .saturating_add(encoded_path.len())
                .saturating_add(1)
                > max_path_bytes
            {
                continue;
            }
            paths.push_str(separator);
            paths.push_str(&encoded_path);
        }
        paths.push(']');
        format!("{TRUSTED_SKILLS_PREFIX}{paths}")
    }
}

impl std::fmt::Debug for TrustedSkills {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TrustedSkills")
            .field("count", &self.paths.len())
            .finish_non_exhaustive()
    }
}

pub(crate) struct TrustedSkillsSection;

impl SectionContributor for TrustedSkillsSection {
    fn scope(&self) -> SectionScope {
        SectionScope::AsyncOnly
    }
    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError> {
        Ok((!input.trusted_skill_paths.is_empty()).then(|| {
            ContextSection::TrustedSkills(TrustedSkills {
                paths: input.trusted_skill_paths.to_vec(),
            })
        }))
    }
}

#[cfg(test)]
#[path = "trusted_skills_tests.rs"]
mod tests;
