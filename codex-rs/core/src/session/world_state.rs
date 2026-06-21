use super::PreviousTurnSettings;
use super::Session;
use super::turn_context::TurnContext;
use crate::context::EnvironmentsState;
use crate::context::world_state::CollaborationModeState;
use crate::context::world_state::ModelState;
use crate::context::world_state::MultiAgentModeState;
use crate::context::world_state::PermissionsState;
use crate::context::world_state::PersonalityState;
use crate::context::world_state::RealtimeState;
use crate::context::world_state::WorldState;
use crate::environment_selection::TurnEnvironmentSnapshot;
use codex_execpolicy::Policy;
use codex_features::Feature;
use codex_protocol::protocol::TurnContextItem;
use std::sync::Arc;

fn build_world_state_from_turn_context_with_environments(
    turn_context: &TurnContext,
    environments: &TurnEnvironmentSnapshot,
    exec_policy: &Policy,
    personality_feature_enabled: bool,
    environment_subagents: &str,
) -> WorldState {
    let mut world_state = WorldState::default();
    world_state.add_section(ModelState::from_turn_context(turn_context));
    world_state.add_section(PermissionsState::from_turn_context(
        turn_context,
        exec_policy,
    ));
    world_state.add_section(CollaborationModeState::from_turn_context(turn_context));
    world_state.add_section(MultiAgentModeState::from_turn_context(turn_context));
    world_state.add_section(RealtimeState::from_turn_context(turn_context));
    world_state.add_section(PersonalityState::from_turn_context(
        turn_context,
        personality_feature_enabled,
    ));
    if turn_context.config.include_environment_context {
        world_state.add_section(
            EnvironmentsState::from_turn_context_with_environments(turn_context, environments)
                .with_subagents(environment_subagents.to_string()),
        );
    }
    world_state
}

pub(crate) fn build_world_state_from_turn_context_item(
    turn_context_item: &TurnContextItem,
    previous_turn_settings: Option<&PreviousTurnSettings>,
) -> WorldState {
    let mut world_state = WorldState::default();
    if let Some(previous_turn_settings) = previous_turn_settings {
        world_state.add_section(ModelState::from_previous_model(
            &previous_turn_settings.model,
        ));
    }
    world_state.add_section(PermissionsState::from_turn_context_item(turn_context_item));
    world_state.add_section(CollaborationModeState::from_turn_context_item(
        turn_context_item,
    ));
    world_state.add_section(MultiAgentModeState::from_turn_context_item(
        turn_context_item,
    ));
    world_state.add_section(RealtimeState::from_previous(
        turn_context_item.realtime_active,
        previous_turn_settings.and_then(|settings| settings.realtime_active),
    ));
    world_state.add_section(PersonalityState::from_turn_context_item(turn_context_item));
    world_state.add_section(EnvironmentsState::from_turn_context_item(turn_context_item));
    world_state
}

impl Session {
    pub(super) fn build_world_state(&self, turn_context: &TurnContext) -> WorldState {
        let exec_policy = self.services.exec_policy.current();
        build_world_state_from_turn_context_with_environments(
            turn_context,
            &turn_context.environments,
            exec_policy.as_ref(),
            self.features.enabled(Feature::Personality),
            "",
        )
    }

    pub(super) fn build_initial_world_state(
        &self,
        turn_context: &TurnContext,
        environment_subagents: &str,
        base_instructions: &str,
    ) -> WorldState {
        let personality_is_baked = turn_context.personality.is_some_and(|personality| {
            turn_context.model_info.supports_personality()
                && base_instructions
                    == turn_context
                        .model_info
                        .get_model_instructions(Some(personality))
        });
        let exec_policy = self.services.exec_policy.current();
        build_world_state_from_turn_context_with_environments(
            turn_context,
            &turn_context.environments,
            exec_policy.as_ref(),
            self.features.enabled(Feature::Personality) && !personality_is_baked,
            environment_subagents,
        )
    }

    async fn build_live_world_state(&self, turn_context: &TurnContext) -> WorldState {
        let environments = self.services.turn_environments.snapshot().await;
        let exec_policy = self.services.exec_policy.current();
        build_world_state_from_turn_context_with_environments(
            turn_context,
            &environments,
            exec_policy.as_ref(),
            self.features.enabled(Feature::Personality),
            "",
        )
    }

    pub(crate) async fn record_world_state_diff(&self, turn_context: &TurnContext) {
        let world_state = Arc::new(self.build_live_world_state(turn_context).await);
        let previous = {
            let state = self.state.lock().await;
            state.world_state()
        };
        let fragments = match previous.as_deref() {
            Some(previous) => world_state.render_diff(previous),
            None => world_state.render_full(),
        };
        let items = crate::context_manager::updates::merge_contextual_fragments(fragments);
        if !items.is_empty() {
            self.record_conversation_items(turn_context, &items).await;
        }
        let mut state = self.state.lock().await;
        state.set_world_state(world_state);
    }
}
