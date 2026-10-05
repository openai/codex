//! Persist the user's Daybreak selection for the active thread and new threads.

use super::*;

impl App {
    pub(super) async fn persist_daybreak_selection(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        enabled: bool,
    ) {
        if !self
            .chat_widget
            .config_ref()
            .features
            .enabled(Feature::CliDaybreak)
            || self.active_thread_id != Some(thread_id)
        {
            return;
        }
        if enabled
            && (!self.chat_widget.daybreak_turn_eligible(enabled)
                || !crate::daybreak::available(&self.chat_widget.model_catalog().models))
        {
            self.chat_widget.add_error_message(
                "Daybreak availability could not be confirmed for this account.".into(),
            );
            return;
        }
        match app_server
            .thread_read(thread_id, /*include_turns*/ false)
            .await
        {
            Ok(thread) if !thread.ephemeral => {
                let request_id = app_server.next_request_id();
                let response: Result<codex_app_server_protocol::ThreadMetadataUpdateResponse, _> =
                    app_server
                        .request_handle()
                        .request_typed(ClientRequest::ThreadMetadataUpdate {
                            request_id,
                            params: codex_app_server_protocol::ThreadMetadataUpdateParams {
                                thread_id: thread_id.to_string(),
                                daybreak_enabled: Some(enabled),
                                project_id: None,
                                git_info: None,
                            },
                        })
                        .await;
                if let Err(error) = response {
                    self.chat_widget.add_error_message(format!(
                        "Failed to save Daybreak for this thread: {error}"
                    ));
                    return;
                }
            }
            Ok(_) => {}
            Err(error) => {
                self.chat_widget.add_error_message(format!(
                    "Failed to read the thread to save Daybreak: {error}"
                ));
                return;
            }
        }
        self.chat_widget.set_daybreak_enabled(enabled);
        if self.primary_thread_id == Some(thread_id)
            && let Some(session) = self.primary_session_configured.as_mut()
        {
            session.daybreak_enabled = enabled;
        }
        if let Some(channel) = self.thread_event_channels.get(&thread_id) {
            let mut store = channel.store.lock().await;
            if let Some(session) = store.session.as_mut() {
                session.daybreak_enabled = enabled;
            }
        }
        self.refresh_status_line();
        let edits = vec![crate::config_update::replace_config_value(
            "daybreak",
            serde_json::json!(enabled),
        )];
        match self
            .persist_model_defaults(
                app_server.request_handle(),
                edits,
                "default Daybreak preference",
            )
            .await
        {
            Ok(()) => match crate::config_update::read_effective_config(
                app_server.request_handle(),
                self.chat_widget
                    .config_ref()
                    .cwd
                    .to_string_lossy()
                    .into_owned(),
            )
            .await
            {
                Ok(response) => {
                    self.config.daybreak_enabled = response
                        .config
                        .additional
                        .get("daybreak")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                }
                Err(error) => self.chat_widget.add_error_message(format!(
                    "Failed to read the default Daybreak preference: {error}"
                )),
            },
            Err(error) => self.chat_widget.add_error_message(format!(
                "Failed to save the default Daybreak preference: {error}"
            )),
        }
        self.chat_widget.add_info_message(
            format!(
                "Daybreak {}. Applies to new turns.",
                if enabled { "on" } else { "off" }
            ),
            /*hint*/ None,
        );
    }
}
