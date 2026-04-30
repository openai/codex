//! Chat-widget wiring for the `/ide` command and IDE context prompt injection.

use std::path::Path;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::user_input::UserInput;

use super::ChatWidget;
use crate::bottom_pane::IdeContextStatusIndicator;
use crate::ide_context::IdeContext;
use crate::ide_context::IdeContextClient;
use crate::ide_context::IdeContextError;

const IDE_CONTEXT_RECENT_TOGGLE_RETRY_WINDOW: Duration = Duration::from_secs(5);
const IDE_CONTEXT_RECENT_TOGGLE_RETRY_DELAY: Duration = Duration::from_millis(250);
const IDE_CONTEXT_RECENT_TOGGLE_RETRY_ATTEMPTS: usize = 12;

#[derive(Default)]
pub(super) struct IdeContextState {
    enabled: bool,
    prompt_fetch_warned: bool,
    last_disabled_at: Option<Instant>,
    client: Option<IdeContextClient>,
}

impl IdeContextState {
    pub(super) fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn enable(&mut self) {
        self.enabled = true;
        self.prompt_fetch_warned = false;
    }

    fn disable(&mut self) {
        self.enabled = false;
        self.prompt_fetch_warned = false;
        self.last_disabled_at = Some(Instant::now());
        self.client = None;
    }

    fn mark_available(&mut self) {
        self.prompt_fetch_warned = false;
        self.last_disabled_at = None;
    }

    fn status_indicator(&self) -> Option<IdeContextStatusIndicator> {
        if !self.enabled {
            return None;
        }

        Some(IdeContextStatusIndicator::Active)
    }

    fn should_retry_recent_toggle(&self) -> bool {
        self.last_disabled_at.is_some_and(|disabled_at| {
            disabled_at.elapsed() <= IDE_CONTEXT_RECENT_TOGGLE_RETRY_WINDOW
        })
    }

    fn fetch_context_for_probe(
        &mut self,
        workspace_root: &Path,
    ) -> Result<IdeContext, IdeContextError> {
        let client = if let Some(client) = self.client.as_mut() {
            client
        } else {
            self.client.insert(IdeContextClient::connect()?)
        };

        let result = client.fetch_ide_context(workspace_root);
        if let Err(err) = &result
            && err.should_reset_client()
        {
            self.client = None;
        }
        result
    }

    fn fetch_context_for_prompt(
        &mut self,
        workspace_root: &Path,
    ) -> Result<IdeContext, IdeContextError> {
        let mut retried_after_reset = false;
        loop {
            let client = if let Some(client) = self.client.as_mut() {
                client
            } else {
                self.client.insert(IdeContextClient::connect_for_prompt()?)
            };

            let result = client.fetch_ide_context_for_prompt(workspace_root);
            match result {
                Ok(context) => return Ok(context),
                Err(err) => {
                    let should_retry =
                        !retried_after_reset && err.should_retry_prompt_fetch_after_reset();
                    if err.should_reset_client() || should_retry {
                        self.client = None;
                    }
                    if should_retry {
                        retried_after_reset = true;
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }
}

impl ChatWidget {
    pub(super) fn handle_ide_command(&mut self) {
        if self.ide_context.is_enabled() {
            self.ide_context.disable();
            self.sync_ide_context_status_indicator();
            self.add_info_message("IDE context is off.".to_string(), /*hint*/ None);
        } else {
            self.ide_context.enable();
            self.add_ide_context_status_message();
        }
    }

    pub(super) fn handle_ide_command_args(&mut self, args: &str) {
        match args.to_ascii_lowercase().as_str() {
            "" => self.handle_ide_command(),
            "on" => {
                self.ide_context.enable();
                self.add_ide_context_status_message();
            }
            "off" => {
                self.ide_context.disable();
                self.sync_ide_context_status_indicator();
                self.add_info_message("IDE context is off.".to_string(), /*hint*/ None);
            }
            "status" => {
                self.add_ide_context_status_message();
            }
            _ => {
                self.add_error_message("Usage: /ide [on|off|status]".to_string());
            }
        }
    }

    /// Fetches fresh IDE context for the outgoing user turn and folds it into the prompt.
    pub(super) fn maybe_apply_ide_context(&mut self, items: &mut Vec<UserInput>) {
        if !self.ide_context.is_enabled() {
            return;
        }

        match self.ide_context.fetch_context_for_prompt(&self.config.cwd) {
            Ok(context) => {
                self.ide_context.mark_available();
                self.sync_ide_context_status_indicator();
                crate::ide_context::apply_ide_context_to_user_input(&context, items);
            }
            Err(err) => {
                self.sync_ide_context_status_indicator();
                if !self.ide_context.prompt_fetch_warned {
                    self.ide_context.prompt_fetch_warned = true;
                    self.add_info_message(
                        "IDE context was skipped for this message.".to_string(),
                        Some(err.prompt_skip_hint()),
                    );
                }
            }
        }
    }

    fn add_ide_context_status_message(&mut self) {
        if !self.ide_context.is_enabled() {
            self.sync_ide_context_status_indicator();
            self.add_info_message("IDE context is off.".to_string(), /*hint*/ None);
            return;
        }

        let mut fetch_result = self.ide_context.fetch_context_for_probe(&self.config.cwd);
        if self.ide_context.should_retry_recent_toggle() {
            // The previous IDE context connection may still be winding down.
            for _ in 0..IDE_CONTEXT_RECENT_TOGGLE_RETRY_ATTEMPTS {
                if !matches!(
                    fetch_result,
                    Err(ref err) if err.is_retryable_after_recent_toggle()
                ) {
                    break;
                }
                std::thread::sleep(IDE_CONTEXT_RECENT_TOGGLE_RETRY_DELAY);
                fetch_result = self.ide_context.fetch_context_for_probe(&self.config.cwd);
            }
        }

        match fetch_result {
            Ok(context) => {
                self.ide_context.mark_available();
                self.sync_ide_context_status_indicator();
                if crate::ide_context::has_prompt_context(&context) {
                    self.add_info_message(
                        "IDE context is on.".to_string(),
                        Some(
                            "Future messages will include your current IDE selection and open tabs."
                                .to_string(),
                        ),
                    );
                } else {
                    self.add_info_message(
                        "IDE context is on.".to_string(),
                        Some("Connected to your IDE.".to_string()),
                    );
                }
            }
            Err(err) => {
                self.ide_context.disable();
                self.sync_ide_context_status_indicator();
                self.add_info_message(
                    "IDE context could not be enabled.".to_string(),
                    Some(err.user_facing_hint()),
                );
            }
        }
    }

    pub(super) fn sync_ide_context_status_indicator(&mut self) {
        self.bottom_pane
            .set_ide_context_status_indicator(self.ide_context.status_indicator());
    }
}
