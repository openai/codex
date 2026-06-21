use super::Session;
use super::turn_context::TurnContext;
use crate::context::EnvironmentsState;
use crate::context::world_state::WorldState;
use crate::environment_selection::TurnEnvironmentSnapshot;
use codex_protocol::protocol::TurnContextItem;
use std::sync::Arc;

pub(crate) fn build_world_state_from_turn_context(turn_context: &TurnContext) -> WorldState {
    build_world_state_from_turn_context_with_environments(turn_context, &turn_context.environments)
}

fn build_world_state_from_turn_context_with_environments(
    turn_context: &TurnContext,
    environments: &TurnEnvironmentSnapshot,
) -> WorldState {
    let mut world_state = WorldState::default();
    if turn_context.config.include_environment_context {
        world_state.add_section(EnvironmentsState::from_turn_context_with_environments(
            turn_context,
            environments,
        ));
    }
    world_state
}

pub(crate) fn build_world_state_from_turn_context_item(
    turn_context_item: &TurnContextItem,
) -> WorldState {
    let mut world_state = WorldState::default();
    world_state.add_section(EnvironmentsState::from_turn_context_item(turn_context_item));
    world_state
}

impl Session {
    async fn build_world_state(&self, turn_context: &TurnContext) -> WorldState {
        let environments = self.services.turn_environments.snapshot().await;
        build_world_state_from_turn_context_with_environments(turn_context, &environments)
    }

    pub(crate) async fn record_world_state_diff(&self, turn_context: &TurnContext) {
        let world_state = Arc::new(self.build_world_state(turn_context).await);
        let previous = {
            let state = self.state.lock().await;
            state.world_state()
        };
        let items = match previous.as_deref() {
            Some(previous) => world_state.render_diff(previous),
            None => world_state.render_full(),
        };
        if !items.is_empty() {
            self.record_conversation_items(turn_context, &items).await;
        }
        let mut state = self.state.lock().await;
        state.set_world_state(world_state);
    }
}
