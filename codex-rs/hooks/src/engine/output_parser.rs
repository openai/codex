#[derive(Debug, Clone)]
pub(crate) struct UniversalOutput {
    pub continue_processing: bool,
    pub stop_reason: Option<String>,
    pub suppress_output: bool,
    pub system_message: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionStartOutput {
    pub universal: UniversalOutput,
    pub additional_context: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct StopOutput {
    pub universal: UniversalOutput,
    pub should_block: bool,
    pub reason: Option<String>,
}

use crate::schema::HookUniversalOutputWire;
use crate::schema::SessionStartCommandOutputWire;
use crate::schema::StopCommandOutputWire;
use crate::schema::StopDecisionWire;

pub(crate) fn parse_session_start(stdout: &str) -> Option<SessionStartOutput> {
    let wire: SessionStartCommandOutputWire = parse_json(stdout)?;
    let additional_context = wire
        .hook_specific_output
        .and_then(|output| output.additional_context);
    Some(SessionStartOutput {
        universal: UniversalOutput::from(wire.universal),
        additional_context,
    })
}

pub(crate) fn parse_stop(stdout: &str) -> Option<StopOutput> {
    let wire: StopCommandOutputWire = parse_json(stdout)?;
    Some(StopOutput {
        universal: UniversalOutput::from(wire.universal),
        should_block: matches!(wire.decision, Some(StopDecisionWire::Block)),
        reason: wire.reason,
    })
}

impl From<HookUniversalOutputWire> for UniversalOutput {
    fn from(value: HookUniversalOutputWire) -> Self {
        Self {
            continue_processing: value.r#continue,
            stop_reason: value.stop_reason,
            suppress_output: value.suppress_output,
            system_message: value.system_message,
        }
    }
}

fn parse_json<T>(stdout: &str) -> Option<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    if !value.is_object() {
        return None;
    }
    serde_json::from_value(value).ok()
}
