//! Tracks the prefix included in normal inference, independently of later history appends.
//! These checkpoints are harness metadata; compaction and replay never acknowledge new input.

use codex_history::ResponseItemEnvelope;

#[derive(Debug, Clone, Default)]
pub(super) struct SamplingProgress {
    pub(super) last_sequence: u64,
    sampled_through: u64,
}

impl SamplingProgress {
    pub(super) fn record(&mut self, envelope: &mut ResponseItemEnvelope) {
        let metadata = envelope.metadata.get_or_insert_default();
        let sequence = metadata
            .history_sequence
            .get_or_insert_with(|| self.last_sequence.strict_add(/* rhs */ 1));
        self.last_sequence = self.last_sequence.max(*sequence);
        self.sampled_through = self
            .sampled_through
            .max(metadata.sampled_through_sequence.unwrap_or_default());
        metadata.sampled_through_sequence = Some(self.sampled_through);
    }

    pub(super) fn restore(items: &mut [ResponseItemEnvelope]) -> Self {
        let mut progress = Self::default();
        for metadata in items.iter().filter_map(|item| item.metadata.as_ref()) {
            progress.last_sequence = progress
                .last_sequence
                .max(metadata.history_sequence.unwrap_or_default());
            progress.sampled_through = progress
                .sampled_through
                .max(metadata.sampled_through_sequence.unwrap_or_default());
        }
        progress.last_sequence = progress.last_sequence.max(progress.sampled_through);
        // Legacy inputs have no delivery proof. Give them distinct identities and keep
        // them pending until a normal request includes them; never guess from output order.
        progress.assign_missing_sequences(items);
        progress
    }

    pub(super) fn checkpoint(&mut self, items: &mut [ResponseItemEnvelope]) {
        self.assign_missing_sequences(items);
        if let Some(first) = items.first_mut() {
            first
                .metadata
                .get_or_insert_default()
                .sampled_through_sequence = Some(self.sampled_through);
        }
    }

    fn assign_missing_sequences(&mut self, items: &mut [ResponseItemEnvelope]) {
        for envelope in items {
            if envelope
                .metadata
                .as_ref()
                .and_then(|m| m.history_sequence)
                .is_none()
            {
                self.record(envelope);
            }
        }
    }
}

pub(crate) fn sampled_through(items: &[ResponseItemEnvelope]) -> u64 {
    items
        .iter()
        .filter_map(|item| {
            item.metadata
                .as_ref()
                .and_then(|metadata| metadata.sampled_through_sequence)
        })
        .max()
        .unwrap_or_default()
}
