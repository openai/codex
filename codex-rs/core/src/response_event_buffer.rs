use std::collections::BTreeMap;

use codex_protocol::models::ResponseItem;

/// Restores response order for completed items without delaying incremental events.
#[derive(Default)]
pub(crate) struct OutputItemDoneBuffer {
    next_output_index: usize,
    pending: BTreeMap<usize, ResponseItem>,
}

impl OutputItemDoneBuffer {
    pub(crate) fn push(
        &mut self,
        item: ResponseItem,
        output_index: Option<usize>,
    ) -> Vec<(ResponseItem, Option<usize>)> {
        let Some(output_index) = output_index else {
            return vec![(item, None)];
        };

        if output_index < self.next_output_index {
            return vec![(item, Some(output_index))];
        }

        self.pending.insert(output_index, item);
        self.take_ready()
    }

    pub(crate) fn finish(&mut self) -> Vec<(ResponseItem, Option<usize>)> {
        let pending = std::mem::take(&mut self.pending);
        pending
            .into_iter()
            .map(|(output_index, item)| (item, Some(output_index)))
            .collect()
    }

    fn take_ready(&mut self) -> Vec<(ResponseItem, Option<usize>)> {
        let mut ready = Vec::new();
        while let Some(item) = self.pending.remove(&self.next_output_index) {
            ready.push((item, Some(self.next_output_index)));
            self.next_output_index += 1;
        }
        ready
    }
}
