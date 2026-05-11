/// One executable tool call delivered to a contributed tool.
pub struct ToolCall<C> {
    pub context: C,
    pub call_id: String,
    pub input: ToolInput,
}

/// Model-supplied input for the executable tool families currently exposed by
/// the shared tool seam.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolInput {
    Function { arguments: String },
    Freeform { input: String },
}
