use super::session::Session;
use super::step_context::StepContext;
use crate::context::world_state::AgentsMdState;
use crate::context::world_state::EnvironmentsState;
use crate::context::world_state::WorldState;

impl Session {
    pub(crate) async fn build_world_state_for_step(
        &self,
        step_context: &StepContext,
    ) -> WorldState {
        let turn_context = step_context.turn.as_ref();
        let environment_subagents = if turn_context.config.include_environment_context {
            self.services
                .agent_control
                .format_environment_context_subagents(self.thread_id)
                .await
        } else {
            String::new()
        };

        let mut world_state = WorldState::default();
        world_state.add_section(AgentsMdState::new(step_context.loaded_agents_md.as_deref()));
        if turn_context.config.include_environment_context {
            world_state.add_section(
                EnvironmentsState::from_turn_context_with_environments(
                    turn_context,
                    &step_context.environments,
                )
                .with_subagents(environment_subagents),
            );
        }
        world_state
    }
}
