//! Save the status line provider to the active local user configuration.

use super::App;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::statusline_provider::StatuslineProvider;
use anyhow::Context;
use codex_config::types::BuiltinStatusLine;
use codex_config::types::StatusLineSelection;

impl App {
    pub(super) async fn save_status_line_provider(
        &mut self,
        provider: StatusLineSelection,
        provider_from_picker: Option<StatuslineProvider>,
    ) -> anyhow::Result<()> {
        let value = match &provider {
            StatusLineSelection::Builtin(BuiltinStatusLine::Default) => toml_edit::value("default"),
            StatusLineSelection::Mcp(id) => {
                let table = toml_edit::InlineTable::from_iter([
                    ("server", id.server.as_str()),
                    ("tool", id.tool.as_str()),
                ]);
                toml_edit::value(table)
            }
        };
        ConfigEditsBuilder::for_config_path(self.local_settings.user_config_path.as_path())
            .with_edits([ConfigEdit::SetPath {
                segments: vec!["tui".into(), "status_line_provider".into()],
                value,
            }])
            .apply()
            .await
            .context("Failed to save status line provider")?;

        self.local_settings.tui.status_line_provider = Some(Box::new(provider.clone()));
        self.chat_widget.local_settings.tui.status_line_provider = Some(Box::new(provider));
        self.chat_widget
            .reselect_statusline_provider(provider_from_picker);
        Ok(())
    }
}

#[cfg(test)]
#[path = "status_line_settings_tests.rs"]
mod tests;
