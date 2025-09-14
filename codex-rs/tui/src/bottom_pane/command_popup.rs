use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::WidgetRef;

use super::popup_consts::MAX_POPUP_ROWS;
use super::scroll_state::ScrollState;
use super::selection_popup_common::GenericDisplayRow;
use super::selection_popup_common::render_rows;
use crate::slash_command::SlashCommand;
use crate::slash_command::built_in_slash_commands;
use codex_common::fuzzy_match::fuzzy_match;
use codex_protocol::custom_prompts::CustomPrompt;
// no additional imports
use std::collections::HashSet;

/// A selectable item in the popup: either a built-in command or a user prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandItem {
    Builtin(SlashCommand),
    // Index into `prompts`
    UserPrompt(usize),
}

pub(crate) struct CommandPopup {
    command_filter: String,
    builtins: Vec<(&'static str, SlashCommand)>,
    prompts: Vec<CustomPrompt>,
    state: ScrollState,
}

impl CommandPopup {
    pub(crate) fn new(mut prompts: Vec<CustomPrompt>) -> Self {
        let builtins = built_in_slash_commands();
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

    pub(crate) fn prompt_name(&self, idx: usize) -> Option<&str> {
        self.prompts.get(idx).map(|p| p.name.as_str())
    }

    pub(crate) fn prompt_content(&self, idx: usize) -> Option<&str> {
        self.prompts.get(idx).map(|p| p.content.as_str())
    }

    /// Update the filter string based on the current composer text. The text
    /// passed in is expected to start with a leading '/'. Everything after the
    /// *first* '/" on the *first* line becomes the active filter that is used
    /// to narrow down the list of available commands.
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

    /// Determine the preferred height of the popup for a given width.
    /// Accounts for wrapped descriptions so that long tooltips don't overflow.
    pub(crate) fn calculate_required_height(&self, width: u16) -> u16 {
        use super::selection_popup_common::measure_rows_height;

        let rows = self.rows_from_matches(self.filtered());

        measure_rows_height(&rows, &self.state, MAX_POPUP_ROWS, width)
    }

    /// Compute fuzzy-filtered matches over built-in commands and user prompts,
    /// paired with optional highlight indices and score. Sorted by ascending
    /// score, then by name for stability.
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
        for (idx, p) in self.prompts.iter().enumerate() {
            if let Some((indices, score)) = fuzzy_match(&p.name, filter) {
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

    fn filtered_items(&self) -> Vec<CommandItem> {
        self.filtered().into_iter().map(|(c, _, _)| c).collect()
    }

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
                        (
                            format!("/{}", prompt.name),
                            build_prompt_row_description(prompt),
                        )
                    }
                };
                GenericDisplayRow {
                    name,
                    match_indices: indices.map(|v| v.into_iter().map(|i| i + 1).collect()),
                    is_current: false,
                    description: Some(description),
                }
            })
            .collect()
    }

    /// Move the selection cursor one step up.
    pub(crate) fn move_up(&mut self) {
        let len = self.filtered_items().len();
        self.state.move_up_wrap(len);
        self.state.ensure_visible(len, MAX_POPUP_ROWS.min(len));
    }

    /// Move the selection cursor one step down.
    pub(crate) fn move_down(&mut self) {
        let matches_len = self.filtered_items().len();
        self.state.move_down_wrap(matches_len);
        self.state
            .ensure_visible(matches_len, MAX_POPUP_ROWS.min(matches_len));
    }

    /// Return currently selected command, if any.
    pub(crate) fn selected_item(&self) -> Option<CommandItem> {
        let matches = self.filtered_items();
        self.state
            .selected_idx
            .and_then(|idx| matches.get(idx).copied())
    }
}

impl WidgetRef for CommandPopup {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        let rows = self.rows_from_matches(self.filtered());
        render_rows(
            area,
            buf,
            &rows,
            &self.state,
            MAX_POPUP_ROWS,
            false,
            "no matches",
        );
    }
}

/// Build the display description for a custom prompt row:
///   "<five-word excerpt>  <1> <2> <3>"
/// - Excerpt comes from the first non-empty line in content, cleaned and
///   truncated to five words. Placeholders like $1..$9 and $ARGUMENTS are
///   stripped from the excerpt to avoid noise.
/// - Argument tokens show any referenced positional placeholders ($1..$9) in
///   ascending order as minimal "<n>" hints. `$ARGUMENTS` is intentionally
///   omitted here to keep the UI simple, per product guidance.
fn build_prompt_row_description(prompt: &CustomPrompt) -> String {
    let base = if let Some(d) = &prompt.description {
        description_excerpt(d)
    } else {
        five_word_excerpt(&prompt.content)
    };
    let base = base.unwrap_or_else(|| "send saved prompt".to_string());
    if let Some(hint) = &prompt.argument_hint {
        if hint.is_empty() {
            base
        } else {
            format!("{base}  {hint}")
        }
    } else {
        base
    }
}

fn description_excerpt(desc: &str) -> Option<String> {
    let normalized = desc.replace("\\n", " ");
    five_word_excerpt(&normalized)
}

/// Extract a five-word excerpt from the first non-empty line of `content`.
/// Cleans basic markdown/backticks and removes placeholder tokens.
fn five_word_excerpt(content: &str) -> Option<String> {
    let line = content.lines().map(|l| l.trim()).find(|l| !l.is_empty())?;

    // Strip simple markdown markers and placeholders from the excerpt source.
    let mut cleaned = line.replace(['`', '*', '_'], "");

    // Remove leading markdown header symbols (e.g., "# ").
    if let Some(stripped) = cleaned.trim_start().strip_prefix('#') {
        cleaned = stripped.trim_start_matches('#').trim_start().to_string();
    }

    // Remove placeholder occurrences from excerpt text.
    for n in 1..=9 {
        cleaned = cleaned.replace(&format!("${n}"), "");
    }
    cleaned = cleaned.replace("$ARGUMENTS", "");

    // Remove a small set of common punctuation that can look odd mid-excerpt
    // once placeholders are stripped (keep hyphens and slashes).
    for ch in [',', ';', ':', '!', '?', '(', ')', '{', '}', '[', ']'] {
        cleaned = cleaned.replace(ch, "");
    }

    // Collapse whitespace and split into words.
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }
    let take = words.len().min(5);
    let mut out = words[..take].join(" ");
    if words.len() > 5 {
        out.push('…');
    }
    Some(out)
}

// (no positional arg tokens in the popup)

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    #[test]
    fn filter_includes_init_when_typing_prefix() {
        let mut popup = CommandPopup::new(Vec::new());
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

    #[test]
    fn selecting_init_by_exact_match() {
        let mut popup = CommandPopup::new(Vec::new());
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

    #[test]
    fn model_is_first_suggestion_for_mo() {
        let mut popup = CommandPopup::new(Vec::new());
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
        let popup = CommandPopup::new(prompts);
        let items = popup.filtered_items();
        let mut prompt_names: Vec<String> = items
            .into_iter()
            .filter_map(|it| match it {
                CommandItem::UserPrompt(i) => popup.prompt_name(i).map(ToString::to_string),
                _ => None,
            })
            .collect();
        prompt_names.sort();
        assert_eq!(prompt_names, vec!["bar".to_string(), "foo".to_string()]);
    }

    #[test]
    fn prompt_name_collision_with_builtin_is_ignored() {
        // Create a prompt named like a builtin (e.g. "init").
        let popup = CommandPopup::new(vec![CustomPrompt {
            name: "init".to_string(),
            path: "/tmp/init.md".to_string().into(),
            content: "should be ignored".to_string(),
            description: None,
            argument_hint: None,
        }]);
        let items = popup.filtered_items();
        let has_collision_prompt = items.into_iter().any(|it| match it {
            CommandItem::UserPrompt(i) => popup.prompt_name(i) == Some("init"),
            _ => false,
        });
        assert!(
            !has_collision_prompt,
            "prompt with builtin name should be ignored"
        );
    }

    #[test]
    fn prompt_displays_excerpt_when_placeholders_present() {
        let prompts = vec![CustomPrompt {
            name: "with-args".to_string(),
            path: "/tmp/with-args.md".into(),
            content: "Header $1 and $3; rest: $ARGUMENTS".to_string(),
            description: None,
            argument_hint: None,
        }];
        let mut popup = CommandPopup::new(prompts);
        // Filter so the prompt appears at the top and within visible rows.
        popup.on_composer_text_change("/with-args".to_string());

        // Render a buffer tall enough to show the selection row.
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        popup.render_ref(Rect::new(0, 0, 80, 10), &mut buf);
        let screen = buffer_to_string(&buf);
        // Expect only the excerpt (first five words without placeholders).
        assert!(
            screen.contains("Header and rest"),
            "expected five-word excerpt; got:\n{screen}"
        );
        assert!(
            screen.contains("/with-args"),
            "expected command label; got:\n{screen}"
        );
    }

    #[test]
    fn prompt_uses_excerpt_when_no_placeholders_present() {
        let prompts = vec![CustomPrompt {
            name: "no-args".to_string(),
            path: "/tmp/no-args.md".into(),
            content: "plain content".to_string(),
            description: None,
            argument_hint: None,
        }];
        let mut popup = CommandPopup::new(prompts);
        popup.on_composer_text_change("/no-args".to_string());

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        popup.render_ref(Rect::new(0, 0, 80, 10), &mut buf);
        let screen = buffer_to_string(&buf);
        assert!(
            screen.contains("plain content"),
            "expected excerpt fallback; got:\n{screen}"
        );
    }

    #[test]
    fn prompt_uses_frontmatter_description_and_argument_hint_when_present() {
        let prompts = vec![CustomPrompt {
            name: "review-pr".to_string(),
            path: "/tmp/review-pr.md".into(),
            content: "Summarize changes $1".to_string(),
            description: Some("Review a PR with context".to_string()),
            argument_hint: Some("[pr-number] [priority]".to_string()),
        }];
        let mut popup = CommandPopup::new(prompts);
        popup.on_composer_text_change("/review-pr".to_string());

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        popup.render_ref(Rect::new(0, 0, 80, 10), &mut buf);
        let screen = buffer_to_string(&buf);
        assert!(screen.contains("/review-pr"));
        assert!(screen.contains("Review a PR with context  [pr-number] [priority]"));
    }

    fn buffer_to_string(buf: &Buffer) -> String {
        let area = buf.area;
        let mut s = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                let cell = &buf[(x, y)];
                s.push(cell.symbol().chars().next().unwrap_or(' '));
            }
            s.push('\n');
        }
        s
    }
}
