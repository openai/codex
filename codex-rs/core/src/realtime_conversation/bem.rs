use codex_protocol::models::MessagePhase;
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) fn message_phase(
    text: &str,
    channel_prefixes: &BTreeMap<String, Vec<String>>,
) -> Option<MessagePhase> {
    for (channel, default_prefix, phase) in [
        ("analysis", "[ANALYSIS]", MessagePhase::Commentary),
        ("commentary", "[COMMENTARY]", MessagePhase::Commentary),
        ("final", "[FINAL]", MessagePhase::FinalAnswer),
    ] {
        let matches = channel_prefixes.get(channel).map_or_else(
            || text.starts_with(default_prefix),
            |prefixes| {
                prefixes
                    .iter()
                    .any(|prefix| !prefix.is_empty() && text.starts_with(prefix))
            },
        );
        if matches {
            return Some(phase);
        }
    }

    None
}

/// Buffers a streamed BEM message until its channel header is complete.
///
/// Once the channel is known, the original envelope is released unchanged so
/// the frontend model can distinguish BEM `analysis` from `commentary`.
#[derive(Debug, Default)]
pub(super) struct ChannelParser {
    channel_prefixes: Arc<BTreeMap<String, Vec<String>>>,
    buffered_text: String,
    phase: Option<MessagePhase>,
}

impl ChannelParser {
    pub(super) fn new(channel_prefixes: Arc<BTreeMap<String, Vec<String>>>) -> Self {
        Self {
            channel_prefixes,
            ..Self::default()
        }
    }

    pub(super) fn push(&mut self, text: &str) -> Option<String> {
        if self.phase.is_some() {
            return Some(text.to_string());
        }

        self.buffered_text.push_str(text);
        self.phase = message_phase(&self.buffered_text, &self.channel_prefixes);
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
