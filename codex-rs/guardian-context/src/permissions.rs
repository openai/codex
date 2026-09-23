//! Sync permission evidence formatting. The host resolves the reviewed environment
//! and its filesystem policy; this section neither resolves nor relaxes restrictions.

use crate::ContextSection;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionInput;
use crate::SectionScope;
use codex_context_fragments::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;

/// Display paths and globs denied by the reviewed environment's active policy.
/// These are evidence strings, not filesystem paths to resolve in the reviewer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PermissionContext {
    pub environment_id: Option<String>,
    pub denied_paths: Vec<String>,
    pub denied_globs: Vec<String>,
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

    fn body(&self) -> String {
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
            Some(environment_id) => format!(
                "For this action on environment {environment_id:?}, the active permission profile"
            ),
            None => "The parent turn's active permission profile".to_string(),
        };
        if entries.is_empty() {
            format!("{scope} has no explicit denied-read paths/globs.\n")
        } else {
            format!(
                "{scope} denies reading these paths/globs. These are policy restrictions; do not approve escalation whose purpose is to read them.\n{}\n",
                entries.join("\n")
            )
        }
    }
}

pub(crate) struct PermissionContextSection;

impl SectionContributor for PermissionContextSection {
    fn scope(&self) -> SectionScope {
        SectionScope::SyncOnly
    }

    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError> {
        let Some(permissions) = input.permissions else {
            return Ok(None);
        };
        if permissions.environment_id.is_none()
            && permissions.denied_paths.is_empty()
            && permissions.denied_globs.is_empty()
        {
            return Ok(None);
        }
        let (start, end) = permissions.markers();
        Ok(Some(ContextSection::PermissionContext {
            items: vec![start.into(), permissions.body(), end.into()],
        }))
    }
}
