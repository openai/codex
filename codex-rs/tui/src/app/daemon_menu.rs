//! Local daemon maintenance only records an update action after explicit confirmation.
//! The CLI executes it in the foreground after the TUI restores the terminal.

use super::*;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::update_action::DaemonUpdateSource;
use crate::wrapping::word_wrap_lines;
use ratatui::buffer::Buffer;
use ratatui::widgets::Paragraph;

struct DaemonMenuHeader(Vec<Line<'static>>);

impl Renderable for DaemonMenuHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Renderable::render(
            &Paragraph::new(word_wrap_lines(&self.0, usize::from(area.width))),
            area,
            buf,
        );
    }

    fn desired_height(&self, width: u16) -> u16 {
        word_wrap_lines(&self.0, usize::from(width)).len() as u16
    }
}

impl App {
    pub(super) fn open_daemon_menu(&mut self) {
        let status = self
            .chat_widget
            .remote_connection
            .as_ref()
            .filter(|_| matches!(self.app_server_target, AppServerTarget::LocalDaemon { .. }))
            .map(|connection| format!("Running daemon: {}", connection.version))
            .unwrap_or_else(|| "Not connected to the local background server.".to_string());
        let mut header = vec![Line::from("Daemon".bold()), Line::from(status.dim())];
        let unavailable = if matches!(self.app_server_target, AppServerTarget::Remote { .. }) {
            Some(
                "Manage this server on its host. Local daemon updates are unavailable for remote connections.",
            )
        } else if self.daemon_cli_executable.is_none() {
            Some("Run the Codex CLI to manage the daemon from this menu.")
        } else {
            None
        };
        if let Some(guidance) = unavailable {
            header.push(Line::from(guidance.dim()));
        }
        let has_package = self.daemon_cli_executable.as_ref().is_some_and(|path| {
            codex_install_context::InstallContext::from_exe(
                cfg!(target_os = "macos"),
                Some(path.as_path()),
                /*method_override*/ None,
            )
            .package_layout
            .is_some()
        });
        let items = [
            (
                DaemonUpdateSource::PublicStable,
                "Install latest public stable",
            ),
            (DaemonUpdateSource::ThisCli, "Use this CLI build"),
        ]
        .into_iter()
        .map(|(source, name)| SelectionItem {
            name: name.to_string(),
            is_disabled: unavailable.is_some(),
            disabled_reason: (unavailable.is_none()
                && source == DaemonUpdateSource::ThisCli
                && !has_package)
                .then(|| "This CLI has no local package to copy.".to_string()),
            actions: vec![Box::new(move |tx| {
                tx.send(AppEvent::ConfirmDaemonUpdate(source));
            })],
            dismiss_on_select: true,
            ..Default::default()
        })
        .collect();
        self.chat_widget.show_selection_view(SelectionViewParams {
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            header: Box::new(DaemonMenuHeader(header)),
            items,
            ..SelectionViewParams::picker()
        });
    }

    pub(super) fn confirm_daemon_update(&mut self, source: DaemonUpdateSource) {
        let Some(executable) = &self.daemon_cli_executable else {
            return;
        };
        if matches!(self.app_server_target, AppServerTarget::Remote { .. }) {
            return;
        }
        let mut explanation = match source {
            DaemonUpdateSource::PublicStable => "Install the latest public stable release. Restore production updates; keep your automatic-update setting.".to_string(),
            DaemonUpdateSource::ThisCli => {
                let version = codex_install_context::InstallContext::current()
                    .package_manifest()
                    .map_or_else(|| CODEX_CLI_VERSION.to_string(), |manifest| manifest.version.to_string());
                format!("Use this CLI package v{version} from {}. Copy the complete package and pin it against automatic updates.", executable.display())
            }
        };
        explanation.push_str("\nThis may restart the daemon and interrupt active or queued work.\nCodex exits to update in this terminal. Relaunch it afterward.");
        let mut header = vec![Line::from("Update daemon and exit Codex?".bold())];
        header.extend(explanation.lines().map(|line| Line::from(line.to_owned())));
        self.chat_widget.show_selection_view(SelectionViewParams {
            appearance: crate::bottom_pane::SelectionAppearance::Picker,
            header: Box::new(DaemonMenuHeader(header)),
            items: vec![
                SelectionItem {
                    name: "Cancel".to_string(),
                    dismiss_on_select: true,
                    ..Default::default()
                },
                SelectionItem {
                    name: "Update and exit".to_string(),
                    actions: vec![Box::new(move |tx| {
                        tx.send(AppEvent::RunDaemonUpdate(source))
                    })],
                    require_explicit_confirmation: true,
                    dismiss_on_select: true,
                    ..Default::default()
                },
            ],
            ..SelectionViewParams::picker()
        });
    }
}

#[cfg(test)]
#[path = "daemon_menu_tests.rs"]
mod tests;
