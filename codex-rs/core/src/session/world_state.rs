use super::turn_context::TurnContext;
use crate::context::EnvironmentsState;
use crate::context::world_state::WorldState;
use codex_protocol::protocol::TurnContextItem;

pub(crate) fn build_world_state_from_turn_context(
    turn_context: &TurnContext,
    environment_subagents: &str,
) -> WorldState {
    let mut world_state = WorldState::default();
    if turn_context.config.include_environment_context {
        world_state.add_section(
            EnvironmentsState::from_turn_context(turn_context)
                .with_subagents(environment_subagents.to_string()),
        );
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
