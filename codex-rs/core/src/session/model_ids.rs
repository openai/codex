//! Creates this thread's item and window sequences from the history it will actually replay.

use std::sync::Arc;

use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_protocol::ContextWindowId;
use codex_protocol::ItemIdGenerator;

use super::session::Session;
use crate::state::AutoCompactWindowIds;
use crate::state::ContextWindowIdGenerator;

pub(super) struct InitialModelIds {
    pub item_id_generator: ItemIdGenerator,
    pub context_window_id_generator: ContextWindowIdGenerator,
    pub initial_auto_compact_window_ids: AutoCompactWindowIds,
}

pub(super) fn from_history(history: &InitialHistory) -> InitialModelIds {
    // A resume already owns its window. Allocating a provisional one would skip an ordinal.
    let resumed_window = match history {
        InitialHistory::Resumed(resumed) => {
            resumed
                .history
                .iter()
                .rev()
                .find_map(|record| match record {
                    RolloutItem::Compacted(checkpoint) => {
                        checkpoint.window_id.as_ref()?.validated()
                    }
                    RolloutItem::SessionMeta(meta) if meta.meta.id == resumed.conversation_id => {
                        meta.meta.context_window.as_ref()?.window_id.validated()
                    }
                    _ => None,
                })
        }
        InitialHistory::New | InitialHistory::Cleared | InitialHistory::Forked(_) => None,
    };
    let initialize = |item_id_generator, context_window_id_generator: ContextWindowIdGenerator| {
        let initial = resumed_window.unwrap_or_else(|| context_window_id_generator());
        InitialModelIds {
            item_id_generator,
            context_window_id_generator,
            initial_auto_compact_window_ids: AutoCompactWindowIds::new_initial(initial),
        }
    };
    initialize(
        Session::new_item_id_generator(),
        Arc::new(ContextWindowId::new_uuid_v7),
    )
}
