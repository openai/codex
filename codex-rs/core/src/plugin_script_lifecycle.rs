use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;

use chrono::SecondsFormat;
use chrono::Utc;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::CodexPluginScriptLifecycleEvent;
use codex_analytics::PluginScriptLifecycleStatus;
use codex_analytics::PluginScriptSkill;
use codex_features::Feature;
use codex_utils_absolute_path::AbsolutePathBuf;
use uuid::Uuid;

use crate::plugin_script_resolver::resolve_plugin_script;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::shell::ShellType;

/// Tracks one resolved plugin-script invocation through executor lifecycle events.
///
/// Resolution is dormant until an executor adapter calls these methods. The
/// started fact is built only after successful process spawn so terminal
/// duration measures actual process lifetime rather than attribution time.
pub(crate) struct PluginScriptExecution {
    analytics: AnalyticsEventsClient,
    event: PluginScriptEvent,
    started_at: OnceLock<Instant>,
    terminal_emitted: AtomicBool,
    interrupted: AtomicBool,
    #[cfg(test)]
    emitted: std::sync::Mutex<Vec<CodexPluginScriptLifecycleEvent>>,
}

/// Terminal process result used to classify one plugin script execution.
pub(crate) enum PluginScriptTerminalOutcome {
    Exited { exit_code: i32 },
    Failed { exit_code: Option<i32> },
}

#[derive(Clone)]
struct PluginScriptEvent {
    session_id: String,
    thread_id: String,
    turn_id: String,
    product_client_id: String,
    plugin_id: String,
    execution_id: String,
    script_path: String,
    skill: Option<PluginScriptSkill>,
}

impl PluginScriptExecution {
    pub(crate) fn resolve(
        session: &Session,
        turn: &TurnContext,
        command: &str,
        cwd: &AbsolutePathBuf,
        shell_type: ShellType,
    ) -> Option<Arc<Self>> {
        if !turn.config.features.enabled(Feature::PluginScriptLifecycle) {
            return None;
        }

        let resolved = resolve_plugin_script(
            &turn.first_party_plugin_roots,
            turn.turn_skills.snapshot.outcome(),
            command,
            cwd,
            shell_type,
        )?;
        Some(Arc::new(Self::new(
            session.services.analytics_events_client.clone(),
            PluginScriptEvent {
                session_id: session.session_id().to_string(),
                thread_id: session.thread_id.to_string(),
                turn_id: turn.sub_id.clone(),
                product_client_id: turn.originator.clone(),
                plugin_id: resolved.plugin_id,
                execution_id: Uuid::new_v4().to_string(),
                script_path: resolved.script_path,
                skill: resolved.skill,
            },
        )))
    }

    pub(crate) fn mark_started(&self) {
        if self.started_at.set(Instant::now()).is_err() {
            return;
        }
        self.emit(self.event(
            PluginScriptLifecycleStatus::Started,
            /*duration_ms*/ None,
            /*exit_code*/ None,
        ));
    }

    pub(crate) fn mark_interrupted(&self) {
        self.interrupted.store(true, Ordering::Release);
    }

    pub(crate) fn finish(&self, outcome: PluginScriptTerminalOutcome) {
        let Some(started_at) = self.started_at.get() else {
            return;
        };
        if self.terminal_emitted.swap(true, Ordering::AcqRel) {
            return;
        }

        let (exit_code, terminal_status) = match outcome {
            PluginScriptTerminalOutcome::Exited { exit_code: 0 } => {
                (Some(0), PluginScriptLifecycleStatus::Completed)
            }
            PluginScriptTerminalOutcome::Exited { exit_code } => {
                (Some(exit_code), PluginScriptLifecycleStatus::Failed)
            }
            PluginScriptTerminalOutcome::Failed { exit_code } => {
                (exit_code, PluginScriptLifecycleStatus::Failed)
            }
        };
        let status = if self.interrupted.load(Ordering::Acquire) {
            PluginScriptLifecycleStatus::Interrupted
        } else {
            terminal_status
        };
        let duration_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.emit(self.event(status, Some(duration_ms), exit_code));
    }

    fn new(analytics: AnalyticsEventsClient, event: PluginScriptEvent) -> Self {
        Self {
            analytics,
            event,
            started_at: OnceLock::new(),
            terminal_emitted: AtomicBool::new(false),
            interrupted: AtomicBool::new(false),
            #[cfg(test)]
            emitted: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn event(
        &self,
        status: PluginScriptLifecycleStatus,
        duration_ms: Option<u64>,
        exit_code: Option<i32>,
    ) -> CodexPluginScriptLifecycleEvent {
        CodexPluginScriptLifecycleEvent {
            session_id: self.event.session_id.clone(),
            thread_id: self.event.thread_id.clone(),
            turn_id: self.event.turn_id.clone(),
            product_client_id: self.event.product_client_id.clone(),
            plugin_id: self.event.plugin_id.clone(),
            execution_id: self.event.execution_id.clone(),
            script_path: self.event.script_path.clone(),
            timestamp: lifecycle_timestamp(),
            status,
            duration_ms,
            exit_code,
            skill: self.event.skill.clone(),
        }
    }

    fn emit(&self, event: CodexPluginScriptLifecycleEvent) {
        #[cfg(test)]
        self.emitted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event.clone());
        self.analytics.track_plugin_script_lifecycle(event);
    }
}

fn lifecycle_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
#[path = "plugin_script_lifecycle_tests.rs"]
mod tests;
