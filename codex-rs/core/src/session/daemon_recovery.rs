//! Captures interrupted turns for daemon recovery and restores persisted turn attribution.
//! Callers must flush the rollout after capturing a live turn before persisting its snapshot.

use super::Session;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::TurnEnvironmentSelection;

/// Turn input and turn-start injections have entered the rollout writer.
pub(super) struct RecordedTurnInput;

impl Session {
    pub(crate) async fn recovered_turn_start_options(
        &self,
        turn_id: &str,
    ) -> crate::TurnStartOptions {
        self.state
            .lock()
            .await
            .recovered_turn_start_options(turn_id)
    }

    /// Captures a regular turn only after its input is recorded. The caller must flush the rollout.
    pub(crate) async fn interrupted_turn(
        &self,
    ) -> Option<(String, crate::TurnStartOptions, TurnEnvironmentSelection)> {
        let active = self.active_turn.lock().await;
        let task = active.as_ref()?.task.as_ref()?;
        if task.kind != crate::state::TaskKind::Regular || task.cancellation_token.is_cancelled() {
            return None;
        }
        let context = &task.turn_context;
        context.extension_data.get::<RecordedTurnInput>()?;
        let settings = context.next_step_settings.load();
        let environments = self.services.turn_environments.snapshot_now();
        // Remote identities/configuration are not persisted across daemon restarts.
        if environments.environments.len() != 1 {
            return None;
        }
        let environment = environments.single_local_environment()?.selection();
        if environment.config != EnvironmentConfigState::FromThread {
            return None;
        }
        Some((
            context.sub_id.clone(),
            crate::TurnStartOptions {
                final_output_json_schema: context.final_output_json_schema.clone(),
                service_tier: Some(settings.service_tier.clone().unwrap_or_else(|| {
                    codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string()
                })),
                cyber_access_program: context.cyber_access_program,
                ..Default::default()
            },
            environment,
        ))
    }
}
