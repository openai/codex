//! Keeps the server version notice and local maintenance guidance in the overview.

use super::*;
use crate::update_versions::ServerVersionNoticeKind;

impl App {
    pub(super) fn refresh_server_version_overview_notice(&mut self, client_version: &str) {
        if !self.local_settings.tui.show_server_version_notice {
            self.pending_server_version_notice = None;
            self.reconnect.seen_version_notice = None;
        }
        let server_version = self
            .chat_widget
            .remote_connection
            .as_ref()
            .and_then(|connection| connection.version.strip_prefix('v'))
            .map(str::to_owned);
        let _ = self.initialize_server_version_notice(client_version, server_version.as_deref());
    }

    pub(super) fn initialize_server_version_notice(
        &mut self,
        client_version: &str,
        server_version: Option<&str>,
    ) -> Option<String> {
        let notice = if matches!(self.app_server_target, AppServerTarget::Embedded) {
            None
        } else {
            crate::status::remote_connection::server_version_notice_for_tui(
                &self.local_settings.tui,
                client_version,
                server_version,
            )
        };
        self.update_server_version_overview_notice(
            client_version,
            notice.as_ref().and(server_version),
        );
        notice
    }

    pub(super) fn update_server_version_overview_notice(
        &mut self,
        client_version: &str,
        server_version: Option<&str>,
    ) {
        self.agents_overview
            .view_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .server_version_notice = server_version.and_then(|server| {
            let guidance = if matches!(self.app_server_target, AppServerTarget::LocalDaemon { .. })
            {
                " · /daemon"
            } else {
                ""
            };
            let comparison =
                match crate::update_versions::server_version_notice_kind(client_version, server)? {
                    ServerVersionNoticeKind::Older => "<",
                    ServerVersionNoticeKind::Different => "≠",
                };
            Some(format!(
                "Service v{server} {comparison} Codex CLI v{client_version}{guidance}"
            ))
        });
    }
}
