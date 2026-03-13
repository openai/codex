use super::*;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum KernelToHost {
    ExecLog {
        id: String,
        text: String,
    },
    ExecResult {
        id: String,
        ok: bool,
        output: String,
        #[serde(default)]
        error: Option<String>,
    },
    RunTool(RunToolRequest),
    EmitImage(EmitImageRequest),
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum HostToKernel {
    Exec {
        id: String,
        code: String,
        #[serde(default)]
        timeout_ms: Option<u64>,
        #[serde(default)]
        stream_logs: bool,
    },
    RunToolResult(RunToolResult),
    EmitImageResult(EmitImageResult),
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RunToolRequest {
    pub(super) id: String,
    pub(super) exec_id: String,
    pub(super) tool_name: String,
    pub(super) arguments: String,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct RunToolResult {
    pub(super) id: String,
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) response: Option<JsonValue>,
    #[serde(default)]
    pub(super) error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct EmitImageRequest {
    pub(super) id: String,
    pub(super) exec_id: String,
    pub(super) image_url: String,
    #[serde(default)]
    pub(super) detail: Option<ImageDetail>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct EmitImageResult {
    pub(super) id: String,
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) error: Option<String>,
}

#[derive(Debug)]
pub(super) enum ExecResultMessage {
    Ok {
        content_items: Vec<FunctionCallOutputContentItem>,
    },
    Err {
        message: String,
    },
}
