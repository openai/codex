//! Assembles prepared compaction prefixes under the session-state lock.
//! Validation, suffix retention, and window advancement stay atomic with installation.

use super::CompactionPrefix;
use super::turn_context::TurnContext;
use crate::compact;
use crate::compact::CompactedHistoryMetadata;
use crate::context::world_state::WorldStateSnapshot;
use crate::state::SessionState;
use codex_history::ResponseItemEnvelope;
use codex_login::AuthOwner;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;

pub(crate) struct PrefixCompactionInstallation<'a> {
    pub(crate) prefix: CompactionPrefix,
    pub(crate) prefix_context: Vec<ResponseItem>,
    pub(crate) initial_context: Vec<ResponseItemEnvelope>,
    pub(crate) reset_world_state_sections: Vec<&'static str>,
    pub(crate) model_info: &'a ModelInfo,
    pub(crate) compaction_response_id: String,
    pub(crate) reviewer_compaction_hash: Option<String>,
    pub(crate) turn_context: &'a TurnContext,
    pub(crate) started_item: Option<&'a TurnItem>,
}

pub(super) struct CompactedHistoryReplacement<'a> {
    pub(super) items: Vec<ResponseItemEnvelope>,
    pub(super) world_state_baseline: WorldStateSnapshot,
    pub(super) metadata: CompactedHistoryMetadata,
    pub(super) started_item: Option<(&'a TurnContext, &'a TurnItem)>,
}

impl<'a> PrefixCompactionInstallation<'a> {
    /// The caller holds session state through assembly and history replacement.
    pub(super) fn assemble_replacement(
        self,
        state: &mut SessionState,
        mut items: Vec<ResponseItemEnvelope>,
        mut world_state_baseline: WorldStateSnapshot,
        auth_owner: Option<AuthOwner>,
    ) -> Option<CompactedHistoryReplacement<'a>> {
        // Validate immediately before slicing and replacing the captured prefix.
        if state.history.history_version() != self.prefix.generation
            || state.history.annotated_items().len() < self.prefix.len
            || auth_owner != self.prefix.auth_owner
        {
            return None;
        }
        let current = state.history.annotated_items();
        let (replaced, suffix) = current.split_at(self.prefix.len);
        let mut initial_context = self.initial_context;
        initial_context.extend(
            state
                .additional_context
                .restore_compacted_items(replaced, &mut items),
        );
        if suffix.iter().any(|envelope| {
            crate::compact_retained_tool_outputs::is_retained_tool_output(envelope)
                || compact::is_real_user_or_agent_message(&envelope.item)
        }) {
            items.extend(initial_context);
        } else {
            items = compact::insert_initial_context_before_last_real_user_or_summary(
                items,
                initial_context,
            );
        }
        items.splice(
            0..0,
            self.prefix_context
                .into_iter()
                .map(ResponseItemEnvelope::new),
        );
        items.extend_from_slice(suffix);
        // The unchanged suffix already contains updates through this baseline.
        if let Some(checkpoint) = state.history.world_state_checkpoint() {
            world_state_baseline = WorldStateSnapshot::from(&checkpoint.state);
        }
        // Volatile sections must refresh in the new window even if their
        // previous messages and comparison state survived in the suffix.
        for section_id in self.reset_world_state_sections {
            world_state_baseline.remove_section(section_id);
        }
        // Prefix replacement already retains all edits in the live suffix.
        let input_goal_ids = Default::default();
        let (window_number, window_ids) = state.advance_auto_compact_window();
        Some(CompactedHistoryReplacement {
            items,
            world_state_baseline,
            metadata: CompactedHistoryMetadata {
                input_goal_ids,
                message: String::new(),
                window_number,
                window_ids,
                compaction_response_id: Some(self.compaction_response_id),
                compaction_model_hash: self.model_info.comp_hash.clone(),
                reviewer_compaction_hash: self.reviewer_compaction_hash,
            },
            started_item: self.started_item.map(|item| (self.turn_context, item)),
        })
    }
}
