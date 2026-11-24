//! Shared permission evidence formatting. The host resolves the reviewed environment
//! and its filesystem policy; this section neither resolves nor relaxes restrictions.

use crate::ContextSection;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionInput;
use crate::SectionScope;
use codex_context_fragments::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

/// Display paths and globs denied by the reviewed environment's active policy.
/// These are evidence strings, not filesystem paths to resolve in the reviewer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PermissionContext {
    pub environment_id: Option<String>,
    pub denied_paths: Vec<String>,
    pub denied_globs: Vec<String>,
    pub reviewing_escalated_exec: bool,
}

impl ContextualUserFragment for PermissionContext {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("guardian.permissions".to_owned())
    }

    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            "\n>>> PARENT TURN PERMISSION CONTEXT START\n",
            ">>> PARENT TURN PERMISSION CONTEXT END\n",
        )
    }

    fn body(&self) -> ContentItem {
        let entries = self
            .denied_paths
            .iter()
            .map(|path| format!("- path `{path}`"))
            .chain(
                self.denied_globs
                    .iter()
                    .map(|glob| format!("- glob `{glob}`")),
            )
            .collect::<Vec<_>>();
        let scope = match &self.environment_id {
            Some(environment_id) => {
                format!("The active permission profile for environment {environment_id:?}")
            }
            None => "The parent turn's active permission profile".to_string(),
        };
        input_text(if entries.is_empty() {
            format!("{scope} has no explicit denied-read paths/globs.\n")
        } else {
            let after_approval = if self.reviewing_escalated_exec {
                " Approval can grant read and write access across the rest of the filesystem, \
                 including repository metadata. The command stays sandboxed with the same read \
                 denials. A command can encounter a blocked read incidentally and still \
                 complete its intended operation."
            } else {
                ""
            };
            format!(
                "{scope} denies reading these paths/globs. These are policy restrictions; do not approve escalation whose purpose is to read them.{after_approval}\n{}\n",
                entries.join("\n")
            )
        })
    }
}

// Keep async permission evidence below one thousand estimated tokens.
const MAX_ASYNC_PERMISSION_BYTES: usize = 3_000;

pub(crate) struct PermissionContextSection;

impl SectionContributor for PermissionContextSection {
    fn scope(&self) -> SectionScope {
        SectionScope::Shared
    }

    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError> {
        let Some(permissions) = input.permissions else {
            return Ok(None);
        };
        permissions.section(input.target)
    }
}

impl PermissionContext {
    pub(crate) fn section(
        &self,
        target: crate::ContextTarget,
    ) -> Result<Option<ContextSection>, SectionError> {
        if self.environment_id.is_none()
            && self.denied_paths.is_empty()
            && self.denied_globs.is_empty()
        {
            return Ok(None);
        }
        let body = self.body().into_text().unwrap_or_default();
        // Async scoring falls back to synchronous review instead of dropping restrictions.
        if target == crate::ContextTarget::Async && body.len() > MAX_ASYNC_PERMISSION_BYTES {
            return Err(SectionError::EvidenceLimitExceeded {
                section: "permissions",
            });
        }
        let (start, end) = self.markers();
        Ok(Some(ContextSection::PermissionContext {
            items: vec![start.into(), body, end.into()],
        }))
    }
}
