//! Session-only `/title` and `/statusline` command handling.
//!
//! The command handlers update the live widget without changing saved defaults. Explicit
//! overrides are carried with the existing per-thread input state so navigation and reconnects
//! retain the selected layout.

use super::slash_command_args;
use super::*;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StatusSurfaceOverrides {
    pub(super) terminal_title: Option<Vec<String>>,
    pub(super) status_line: Option<Vec<String>>,
    pub(super) status_line_use_colors: Option<bool>,
}

#[derive(Debug, PartialEq, Eq)]
struct StatusLineCommand {
    items: Option<Vec<StatusLineItem>>,
    use_theme_colors: Option<bool>,
}

impl ChatWidget {
    pub(super) fn handle_terminal_title_command_args(&mut self, args: &str) {
        let preview_data = self.terminal_title_preview_data();
        match parse_terminal_title_command(args, &preview_data) {
            Ok(items) => {
                let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
                self.status_surface_overrides.terminal_title = Some(ids.clone());
                self.local_settings.tui.terminal_title = Some(ids);
                self.refresh_terminal_title();
                self.request_redraw();
            }
            Err(error) => self.add_error_message(error),
        }
    }

    pub(super) fn handle_status_line_command_args(&mut self, args: &str) {
        let preview_data = self.status_surface_preview_data();
        match parse_status_line_command(args, &preview_data) {
            Ok(command) => {
                if let Some(items) = command.items {
                    let ids = items.iter().map(ToString::to_string).collect::<Vec<_>>();
                    self.status_surface_overrides.status_line = Some(ids.clone());
                    self.local_settings.tui.status_line = Some(ids);
                }
                if let Some(use_theme_colors) = command.use_theme_colors {
                    self.status_surface_overrides.status_line_use_colors = Some(use_theme_colors);
                    self.local_settings.tui.status_line_use_colors = use_theme_colors;
                }
                self.refresh_status_line();
                self.request_redraw();
            }
            Err(error) => self.add_error_message(error),
        }
    }

    pub(super) fn restore_status_surface_overrides(&mut self, overrides: StatusSurfaceOverrides) {
        if let Some(items) = &overrides.terminal_title {
            self.local_settings.tui.terminal_title = Some(items.clone());
        }
        if let Some(items) = &overrides.status_line {
            self.local_settings.tui.status_line = Some(items.clone());
        }
        if let Some(use_theme_colors) = overrides.status_line_use_colors {
            self.local_settings.tui.status_line_use_colors = use_theme_colors;
        }
        self.status_surface_overrides = overrides;
        self.refresh_status_surfaces();
    }
}

fn parse_terminal_title_command(
    args: &str,
    preview_data: &StatusSurfacePreviewData,
) -> Result<Vec<TerminalTitleItem>, String> {
    let args = slash_command_args::parse_settings(args, SlashCommand::Title)?;
    let items = args
        .positionals
        .iter()
        .map(|item| parse_item(item, "terminal-title", "/title", preview_data))
        .collect::<Result<Vec<_>, _>>()?;
    let clear = args.has_switch("--clear");
    if clear && !items.is_empty() {
        return Err("Cannot combine --clear with terminal-title items.".into());
    }
    Ok(if clear { Vec::new() } else { items })
}

fn parse_status_line_command(
    args: &str,
    preview_data: &StatusSurfacePreviewData,
) -> Result<StatusLineCommand, String> {
    let args = slash_command_args::parse_settings(args, SlashCommand::Statusline)?;
    let items = args
        .positionals
        .iter()
        .map(|item| parse_item(item, "status-line", "/statusline", preview_data))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StatusLineCommand {
        items: status_line_items(items, args.has_switch("--clear"))?,
        use_theme_colors: args.boolean("colors"),
    })
}

fn status_line_items(
    items: Vec<StatusLineItem>,
    clear: bool,
) -> Result<Option<Vec<StatusLineItem>>, String> {
    if clear && !items.is_empty() {
        return Err("Cannot combine --clear with status-line items.".into());
    }
    Ok(if clear {
        Some(Vec::new())
    } else if items.is_empty() {
        None
    } else {
        Some(items)
    })
}

fn parse_item<T>(
    token: &str,
    surface: &str,
    command: &str,
    preview_data: &StatusSurfacePreviewData,
) -> Result<T, String>
where
    T: std::str::FromStr,
{
    if let Ok(item) = token.parse() {
        return Ok(item);
    }

    let five_hour_id = "five-hour-limit";
    let weekly_id = "weekly-limit";
    let five_hour_matches = preview_data
        .rate_limit_item_name(StatusSurfacePreviewItem::FiveHourLimit, five_hour_id)
        == token;
    let weekly_matches = preview_data
        .rate_limit_item_name(StatusSurfacePreviewItem::WeeklyLimit, weekly_id)
        == token;
    let id = match (five_hour_matches, weekly_matches) {
        (true, false) => five_hour_id,
        (false, true) => weekly_id,
        (true, true) => {
            return Err(format!(
                "Ambiguous {surface} item '{token}'. Use '{five_hour_id}' or '{weekly_id}'."
            ));
        }
        (false, false) => {
            return Err(format!(
                "Unknown {surface} item '{token}'. Use {command} to see available items."
            ));
        }
    };
    id.parse().map_err(|_| {
        format!("Unknown {surface} item '{token}'. Use {command} to see available items.")
    })
}
