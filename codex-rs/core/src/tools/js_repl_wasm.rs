use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use crate::function_tool::FunctionCallError;
use serde::Deserialize;

pub(crate) const JS_REPL_PRAGMA_PREFIX: &str = "// codex-js-repl:";

#[derive(Default)]
pub(crate) struct JsReplManager;

impl JsReplManager {
    pub async fn interrupt_turn_exec(&self, _turn_id: &str) -> Result<bool, FunctionCallError> {
        Ok(false)
    }
}

pub(crate) struct JsReplHandle;

impl fmt::Debug for JsReplHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsReplHandle").finish_non_exhaustive()
    }
}

impl JsReplHandle {
    pub(crate) fn with_node_path(
        _node_path: Option<PathBuf>,
        _node_module_dirs: Vec<PathBuf>,
    ) -> Self {
        Self
    }

    pub(crate) async fn manager(&self) -> Result<Arc<JsReplManager>, FunctionCallError> {
        Err(FunctionCallError::RespondToModel(
            "js_repl is unavailable on wasm32".to_string(),
        ))
    }

    pub(crate) fn manager_if_initialized(&self) -> Option<Arc<JsReplManager>> {
        None
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsReplArgs {
    pub code: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

pub(crate) async fn resolve_compatible_node(
    _config_path: Option<&Path>,
) -> Result<PathBuf, String> {
    Err("js_repl is unavailable on wasm32".to_string())
}
