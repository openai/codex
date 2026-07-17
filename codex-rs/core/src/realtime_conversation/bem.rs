use codex_protocol::models::MessagePhase;

const BEM_ASSISTANT_CHANNEL_PREFIX: &str = "<|start|>assistant<|channel|>";
const BEM_MESSAGE_MARKER: &str = "<|message|>";

pub(super) fn message_phase(text: &str) -> Option<MessagePhase> {
    let channel_and_message = text.strip_prefix(BEM_ASSISTANT_CHANNEL_PREFIX)?;
    let (channel, _) = channel_and_message.split_once(BEM_MESSAGE_MARKER)?;
    match channel {
        "analysis" | "commentary" => Some(MessagePhase::Commentary),
        "final" => Some(MessagePhase::FinalAnswer),
        _ => None,
    }
}

/// Buffers a streamed BEM message until its channel header is complete.
///
/// Once the channel is known, the original envelope is released unchanged so
/// the frontend model can distinguish BEM `analysis` from `commentary`.
#[derive(Debug, Default)]
pub(super) struct ChannelParser {
    buffered_text: String,
    phase: Option<MessagePhase>,
}

impl ChannelParser {
    pub(super) fn push(&mut self, text: &str) -> Option<String> {
        if self.phase.is_some() {
            return Some(text.to_string());
        }

        self.buffered_text.push_str(text);
        self.phase = message_phase(&self.buffered_text);
        self.phase.as_ref()?;
        Some(std::mem::take(&mut self.buffered_text))
    }

    pub(super) fn phase(&self) -> Option<MessagePhase> {
        self.phase.clone()
    }

    pub(super) fn finish(&mut self) -> String {
        std::mem::take(&mut self.buffered_text)
    }
}

#[cfg(test)]
#[path = "bem_tests.rs"]
mod tests;
