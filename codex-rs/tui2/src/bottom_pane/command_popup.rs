//! Renders and filters the slash-command selection popup for the composer.
//!
//! This module builds the list of selectable slash commands shown beneath the
//! composer when a line begins with `/`. It merges built-in [`SlashCommand`]
//! entries with user-defined [`CustomPrompt`] items, filters them via fuzzy
//! matching, and exposes a [`WidgetRef`] renderer that delegates row layout to
//! [`selection_popup_common`].
//!
//! The popup owns the active filter token and the scroll/selection state; it
//! does not execute commands or mutate composer text. When there is no filter,
//! built-ins appear first in presentation order, followed by user prompts sorted
//! by name. When a filter is active, matches are sorted by ascending fuzzy
//! score, then by command or prompt name for stability across runs.
//!
//! The list of built-ins is feature-gated to hide commands that the runtime
//! cannot support (for example, elevated sandbox toggles on Windows). Prompt
//! rows default to a generic description when prompt frontmatter is missing so
//! the layout stays stable even with sparse metadata.
//!
//! Custom prompt names that collide with built-in command names are excluded,
//! and the prompt display names always use the `/prompts:` prefix. Highlight
//! indices are computed against the non-slash display string and are shifted by
//! one when rendered to account for the leading `/`.
//!
//! [`selection_popup_common`]: super::selection_popup_common
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use crate::render::Insets;
use crate::render::RectExt;
use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;
use codex_common::fuzzy_match::fuzzy_match;
use codex_protocol::custom_prompts::CustomPrompt;
use codex_protocol::custom_prompts::PROMPTS_CMD_PREFIX;
use std::collections::HashSet;

/// Returns whether the Windows sandbox is present but not fully elevated.
///
/// This is used to suppress the elevate-sandbox command when the degraded
/// sandbox flow should not be surfaced to the user.
fn windows_degraded_sandbox_active() -> bool {
    cfg!(target_os = "windows")
        && codex_core::windows_sandbox::ELEVATED_SANDBOX_NUX_ENABLED
        && codex_core::get_platform_sandbox().is_some()
        && !codex_core::is_windows_elevated_sandbox_enabled()
}

/// A selectable item in the popup: either a built-in command or a user prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandItem {
    /// A built-in slash command defined by the TUI.
    Builtin(SlashCommand),
    /// An index into [`CommandPopup::prompts`].
    UserPrompt(usize),
}

/// Owns the slash-command popup state and filtered command list.
///
/// The popup keeps a mutable filter token derived from the composer input and
/// drives a [`ScrollState`] that tracks the current selection and scroll
/// window. Built-in commands are cached so the list can be filtered quickly and
/// user prompts are stored in sorted order for stable presentation. Selection
/// indices always refer to the filtered list, so callers must treat them as
/// ephemeral and refresh them whenever the filter changes.
pub(crate) struct CommandPopup {
    /// The active filter token derived from the composer, without the `/`.
    ///
    /// This preserves the original case from the composer so the UI can echo
    /// exactly what the user typed even though matching is case-insensitive.
    command_filter: String,
    /// Built-in slash commands paired with their display names.
    ///
    /// The list is cached in presentation order after feature gating so the
    /// unfiltered popup is stable and filtering can reuse the same ordering.
    builtins: Vec<(&'static str, SlashCommand)>,
    /// Custom prompts, filtered for name collisions and sorted by name.
    prompts: Vec<CustomPrompt>,
    /// Scroll and selection state for the filtered command list.
    ///
    /// The selection index always refers to the filtered list, so it must be
    /// clamped whenever the filter token changes.
    state: ScrollState,
}

impl CommandPopup {
    /// Builds a popup with the provided prompts and feature flags.
    ///
    /// Built-in commands are filtered for feature availability (including the
    /// Windows degraded-sandbox state), and custom prompts that collide with
    /// built-in names are dropped. Prompts are sorted by name so the unfiltered
    /// list is stable for users.
    pub(crate) fn new(mut prompts: Vec<CustomPrompt>, skills_enabled: bool) -> Self {
        let allow_elevate_sandbox = windows_degraded_sandbox_active();
        let builtins: Vec<(&'static str, SlashCommand)> = built_in_slash_commands()
            .into_iter()
            .filter(|(_, cmd)| skills_enabled || *cmd != SlashCommand::Skills)
            .filter(|(_, cmd)| allow_elevate_sandbox || *cmd != SlashCommand::ElevateSandbox)
            .collect();

        // Exclude prompts that collide with builtin command names and sort by name.
        let exclude: HashSet<String> = builtins.iter().map(|(n, _)| (*n).to_string()).collect();
        prompts.retain(|p| !exclude.contains(&p.name));
        prompts.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            command_filter: String::new(),
            builtins,
            prompts,
            state: ScrollState::new(),
        }
    }

    /// Replaces the prompt list, filtering collisions with built-ins.
    ///
    /// This keeps the same sort and collision rules as [`CommandPopup::new`]
    /// while preserving the current filter and selection state.
    pub(crate) fn set_prompts(&mut self, mut prompts: Vec<CustomPrompt>) {
        let exclude: HashSet<String> = self
            .builtins
            .iter()
            .map(|(n, _)| (*n).to_string())
            .collect();
        prompts.retain(|p| !exclude.contains(&p.name));
        prompts.sort_by(|a, b| a.name.cmp(&b.name));
        self.prompts = prompts;
    }

    /// Returns the prompt at the provided index, if it still exists.
    pub(crate) fn prompt(&self, idx: usize) -> Option<&CustomPrompt> {
        self.prompts.get(idx)
    }

    /// Updates the active filter based on the current composer text.
    ///
    /// The text is expected to start with a leading `/`. Everything after the
    /// first `/` on the first line becomes the filter token that narrows the
    /// available commands. The filter keeps the original case for display
    /// stability even though command matching is case-insensitive today.
    ///
    /// The update flow is: parse the first token, update the filter, then clamp
    /// and scroll the selection to keep it inside the filtered list.
    pub(crate) fn on_composer_text_change(&mut self, text: String) {
        let first_line = text.lines().next().unwrap_or("");

        if let Some(stripped) = first_line.strip_prefix('/') {
            // Extract the *first* token (sequence of non-whitespace
            // characters) after the slash so that `/clear something` still
            // shows the help for `/clear`.
            let token = stripped.trim_start();
            let cmd_token = token.split_whitespace().next().unwrap_or("");

            // Update the filter keeping the original case (commands are all
            // lower-case for now but this may change in the future).
            self.command_filter = cmd_token.to_string();
        } else {
            // The composer no longer starts with '/'. Reset the filter so the
            // popup shows the *full* command list if it is still displayed
            // for some reason.
            self.command_filter.clear();
        }

        // Reset or clamp selected index based on new filtered list.
        let matches_len = self.filtered_items().len();
        self.state.clamp_selection(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    /// Determines the preferred popup height for the given width.
    ///
    /// This accounts for wrapped descriptions so long tooltips do not overflow
    /// past the available viewport.
    pub(crate) fn calculate_required_height(&self, width: u16) -> u16 {
        use super::selection_popup_common::measure_rows_height;
        let rows = self.rows_from_matches(self.filtered());

        measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, width)
    }

    /// Computes the fuzzy-filtered matches and their highlight metadata.
    ///
    /// Each entry includes the matched item, the optional highlight indices
    /// returned by [`fuzzy_match`], and its score. When a filter is present the
    /// list is sorted by ascending score and then by name for deterministic
    /// ordering. Custom prompts are matched against their `/prompts:name`
    /// display strings so both `name` and `prompts:name` searches work.
    fn filtered(&self) -> Vec<(CommandItem, Option<Vec<usize>>, i32)> {
        let filter = self.command_filter.trim();
        let mut out: Vec<(CommandItem, Option<Vec<usize>>, i32)> = Vec::new();
        if filter.is_empty() {
            // Built-ins first, in presentation order.
            for (_, cmd) in self.builtins.iter() {
                out.push((CommandItem::Builtin(*cmd), None, 0));
            }

            // Then prompts, already sorted by name.
            for idx in 0..self.prompts.len() {
                out.push((CommandItem::UserPrompt(idx), None, 0));
            }
            return out;
        }

        for (_, cmd) in self.builtins.iter() {
            if let Some((indices, score)) = fuzzy_match(cmd.command(), filter) {
                out.push((CommandItem::Builtin(*cmd), Some(indices), score));
            }
        }

        // Support both search styles:
        // - Typing "name" should surface "/prompts:name" results.
        // - Typing "prompts:name" should also work.
        for (idx, p) in self.prompts.iter().enumerate() {
            let display = format!("{PROMPTS_CMD_PREFIX}:{}", p.name);
            if let Some((indices, score)) = fuzzy_match(&display, filter) {
                out.push((CommandItem::UserPrompt(idx), Some(indices), score));
            }
        }

        // When filtering, sort by ascending score and then by name for stability.
        out.sort_by(|a, b| {
            a.2.cmp(&b.2).then_with(|| {
                let an = match a.0 {
                    CommandItem::Builtin(c) => c.command(),
                    CommandItem::UserPrompt(i) => &self.prompts[i].name,
                };
                let bn = match b.0 {
                    CommandItem::Builtin(c) => c.command(),
                    CommandItem::UserPrompt(i) => &self.prompts[i].name,
                };
                an.cmp(bn)
            })
        });
        out
    }

    /// Returns only the matched command items, discarding highlight metadata.
    fn filtered_items(&self) -> Vec<CommandItem> {
        self.filtered().into_iter().map(|(c, _, _)| c).collect()
    }

    /// Builds display rows for the popup from match data.
    ///
    /// Highlight indices from [`fuzzy_match`] are shifted by one to account for
    /// the leading `/` in the rendered command name.
    fn rows_from_matches(
        &self,
        matches: Vec<(CommandItem, Option<Vec<usize>>, i32)>,
    ) -> Vec<GenericDisplayRow> {
        matches
            .into_iter()
            .map(|(item, indices, _)| {
                let (name, description) = match item {
                    CommandItem::Builtin(cmd) => {
                        (format!("/{}", cmd.command()), cmd.description().to_string())
                    }
                    CommandItem::UserPrompt(i) => {
                        let prompt = &self.prompts[i];
                        let description = prompt
                            .description
                            .clone()
                            .unwrap_or_else(|| "send saved prompt".to_string());
                        (
                            format!("/{PROMPTS_CMD_PREFIX}:{}", prompt.name),
                            description,
                        )
                    }
                };
                GenericDisplayRow {
                    name,
                    match_indices: indices.map(|v| v.into_iter().map(|i| i + 1).collect()),
                    display_shortcut: None,
                    description: Some(description),
                    wrap_indent: None,
                }
            })
            .collect()
    }

    /// Moves the selection cursor one step up, wrapping at the top.
    pub(crate) fn move_up(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    /// Moves the selection cursor one step down, wrapping at the bottom.
    pub(crate) fn move_down(&mut self) {
        let matches_len = self.filtered_items().len();
        self.state.move_down_wrap(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    /// Returns the currently selected command, if any.
    pub(crate) fn selected_item(&self) -> Option<CommandItem> {
        let matches = self.filtered_items();
        self.state
            .selected_idx
            .and_then(|idx| matches.get(idx).copied())
    }
}

impl WidgetRef for CommandPopup {
    /// Renders the popup rows into the provided buffer.
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows = self.rows_from_matches(self.filtered());
        render_rows(
            area.inset(Insets::tlbr(0, 2, 0, 0)),
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            "no matches",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Exercises prefix filtering by asserting `/init` remains among matches.
    #[test]
    fn filter_includes_init_when_typing_prefix() {
        let mut popup = CommandPopup::new(Vec::new(), false);

        // Simulate the composer line starting with '/in' so the popup filters
        // matching commands by prefix.
        popup.on_composer_text_change("/in".to_string());

        // Access the filtered list via the selected command and ensure that
        // one of the matches is the new "init" command.
        let matches = popup.filtered_items();
        let has_init = matches.iter().any(|item| match item {
            CommandItem::Builtin(cmd) => cmd.command() == "init",
            CommandItem::UserPrompt(_) => false,
        });
        assert!(
            has_init,
            "expected '/init' to appear among filtered commands"
        );
    }

    /// Verifies that exact matches are selected when the filter is fully typed.
    #[test]
    fn selecting_init_by_exact_match() {
        let mut popup = CommandPopup::new(Vec::new(), false);
        popup.on_composer_text_change("/init".to_string());

        // When an exact match exists, the selected command should be that
        // command by default.
        let selected = popup.selected_item();
        match selected {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "init"),
            Some(CommandItem::UserPrompt(_)) => panic!("unexpected prompt selected for '/init'"),
            None => panic!("expected a selected command for exact match"),
        }
    }

    /// Verifies fuzzy ordering ranks `/model` first for the `/mo` filter.
    #[test]
    fn model_is_first_suggestion_for_mo() {
        let mut popup = CommandPopup::new(Vec::new(), false);
        popup.on_composer_text_change("/mo".to_string());
        let matches = popup.filtered_items();
        match matches.first() {
            Some(CommandItem::Builtin(cmd)) => assert_eq!(cmd.command(), "model"),
            Some(CommandItem::UserPrompt(_)) => {
                panic!("unexpected prompt ranked before '/model' for '/mo'")
            }
            None => panic!("expected at least one match for '/mo'"),
        }
    }

    /// Confirms unfiltered results include every custom prompt in name order.
    #[test]
    fn prompt_discovery_lists_custom_prompts() {
        let prompts = vec![
            CustomPrompt {
                name: "foo".to_string(),
                path: "/tmp/foo.md".to_string().into(),
                content: "hello from foo".to_string(),
                description: None,
                argument_hint: None,
            },
            CustomPrompt {
                name: "bar".to_string(),
                path: "/tmp/bar.md".to_string().into(),
                content: "hello from bar".to_string(),
                description: None,
                argument_hint: None,
            },
        ];
        let popup = CommandPopup::new(prompts, false);
        let items = popup.filtered_items();
        let mut prompt_names: Vec<String> = items
            .into_iter()
            .filter_map(|it| match it {
                CommandItem::UserPrompt(i) => popup.prompt(i).map(|p| p.name.clone()),
                _ => None,
            })
            .collect();
        prompt_names.sort();
        assert_eq!(prompt_names, vec!["bar".to_string(), "foo".to_string()]);
    }

    /// Asserts that prompts colliding with built-in names are dropped.
    #[test]
    fn prompt_name_collision_with_builtin_is_ignored() {
        // Create a prompt named like a builtin (e.g. "init").
        let popup = CommandPopup::new(
            vec![CustomPrompt {
                name: "init".to_string(),
                path: "/tmp/init.md".to_string().into(),
                content: "should be ignored".to_string(),
                description: None,
                argument_hint: None,
            }],
            false,
        );
        let items = popup.filtered_items();
        let has_collision_prompt = items.into_iter().any(|it| match it {
            CommandItem::UserPrompt(i) => popup.prompt(i).is_some_and(|p| p.name == "init"),
            _ => false,
        });
        assert!(
            !has_collision_prompt,
            "prompt with builtin name should be ignored"
        );
    }

    /// Asserts prompt rows use frontmatter descriptions when provided.
    #[test]
    fn prompt_description_uses_frontmatter_metadata() {
        let popup = CommandPopup::new(
            vec![CustomPrompt {
                name: "draftpr".to_string(),
                path: "/tmp/draftpr.md".to_string().into(),
                content: "body".to_string(),
                description: Some("Create feature branch, commit and open draft PR.".to_string()),
                argument_hint: None,
            }],
            false,
        );
        let rows = popup.rows_from_matches(vec![(CommandItem::UserPrompt(0), None, 0)]);
        let description = rows.first().and_then(|row| row.description.as_deref());
        assert_eq!(
            description,
            Some("Create feature branch, commit and open draft PR.")
        );
    }

    /// Asserts prompt rows fall back to a generic description when missing.
    #[test]
    fn prompt_description_falls_back_when_missing() {
        let popup = CommandPopup::new(
            vec![CustomPrompt {
                name: "foo".to_string(),
                path: "/tmp/foo.md".to_string().into(),
                content: "body".to_string(),
                description: None,
                argument_hint: None,
            }],
            false,
        );
        let rows = popup.rows_from_matches(vec![(CommandItem::UserPrompt(0), None, 0)]);
        let description = rows.first().and_then(|row| row.description.as_deref());
        assert_eq!(description, Some("send saved prompt"));
    }

    /// Exercises fuzzy matching that allows non-prefix subsequences.
    #[test]
    fn fuzzy_filter_matches_subsequence_for_ac() {
        let mut popup = CommandPopup::new(Vec::new(), false);
        popup.on_composer_text_change("/ac".to_string());

        let cmds: Vec<&str> = popup
            .filtered_items()
            .into_iter()
            .filter_map(|item| match item {
                CommandItem::Builtin(cmd) => Some(cmd.command()),
                CommandItem::UserPrompt(_) => None,
            })
            .collect();
        assert!(
            cmds.contains(&"compact") && cmds.contains(&"feedback"),
            "expected fuzzy search for '/ac' to include compact and feedback, got {cmds:?}"
        );
    }
}
