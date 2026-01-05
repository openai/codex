use codex_core::config::Config;
use codex_core::models_manager::model_presets::HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG;
use codex_core::models_manager::model_presets::HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use color_eyre::eyre::Result;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingModelMigrationNotice {
    pub(crate) from_model: String,
    pub(crate) to_model: String,
    // Used to respect hide flags even if config changes between scheduling and display.
    #[serde(default)]
    pub(crate) migration_config_key: Option<String>,
    /// Unix timestamp (seconds) when this notice was scheduled. Used to expire stale notices.
    #[serde(default)]
    pub(crate) scheduled_at_unix_seconds: Option<u64>,
}

pub(crate) use prompt_ui::ModelMigrationCopy;
pub(crate) use prompt_ui::ModelMigrationOutcome;
pub(crate) use prompt_ui::ModelMigrationScreen;
pub(crate) use prompt_ui::migration_copy_for_models;

/// Read and clear the one-shot migration notice file, returning the notice if it should be shown.
///
/// If the notice is returned, this also updates `config.notices.model_migrations` to prevent
/// re-scheduling within the current process.
pub(crate) fn take_pending_model_migration_notice(
    config: &mut Config,
) -> Option<PendingModelMigrationNotice> {
    let notice_path = pending_model_migration_notice_path(config);
    let contents = match std::fs::read_to_string(&notice_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(err) => {
            tracing::error!(
                error = %err,
                notice_path = %notice_path.display(),
                "failed to read pending model migration notice"
            );
            return None;
        }
    };

    let notice: PendingModelMigrationNotice = match serde_json::from_str(&contents) {
        Ok(notice) => notice,
        Err(err) => {
            tracing::error!(
                error = %err,
                notice_path = %notice_path.display(),
                "failed to parse pending model migration notice"
            );
            return None;
        }
    };

    if notice_expired(&notice) {
        let _ = std::fs::remove_file(&notice_path);
        return None;
    }

    if let Some(migration_config_key) = notice.migration_config_key.as_deref()
        && migration_prompt_hidden(config, migration_config_key)
    {
        let _ = std::fs::remove_file(&notice_path);
        return None;
    }

    if let Some(seen_target) = config.notices.model_migrations.get(&notice.from_model)
        && seen_target == &notice.to_model
    {
        let _ = std::fs::remove_file(&notice_path);
        return None;
    }

    // Best-effort: clear the one-shot file so it doesn't appear again.
    let _ = std::fs::remove_file(&notice_path);

    config
        .notices
        .model_migrations
        .insert(notice.from_model.clone(), notice.to_model.clone());

    Some(notice)
}

/// Persist the migration notice for the next startup, replacing any existing scheduled notice.
///
/// Scheduling is intentionally independent of session configuration: it uses the user's config
/// (or the default model preset) to determine what to schedule.
pub(crate) fn refresh_pending_model_migration_notice(
    config: &Config,
    available_models: &[ModelPreset],
) {
    let current_model = config
        .model
        .as_deref()
        .filter(|model| !model.is_empty())
        .or_else(|| {
            available_models
                .iter()
                .find(|preset| preset.is_default)
                .map(|preset| preset.model.as_str())
        });

    let Some(current_model) = current_model else {
        clear_pending_model_migration_notice(config);
        return;
    };

    let Some(ModelUpgrade {
        id: target_model,
        migration_config_key,
        ..
    }) = available_models
        .iter()
        .find(|preset| preset.model == current_model)
        .and_then(|preset| preset.upgrade.as_ref())
    else {
        clear_pending_model_migration_notice(config);
        return;
    };

    if migration_prompt_hidden(config, migration_config_key.as_str()) {
        clear_pending_model_migration_notice(config);
        return;
    }

    if available_models
        .iter()
        .all(|preset| preset.model != target_model.as_str())
    {
        clear_pending_model_migration_notice(config);
        return;
    }

    if !should_show_model_migration_notice(
        current_model,
        target_model.as_str(),
        available_models,
        config,
    ) {
        clear_pending_model_migration_notice(config);
        return;
    }

    let notice_path = pending_model_migration_notice_path(config);

    let notice = PendingModelMigrationNotice {
        from_model: current_model.to_string(),
        to_model: target_model.to_string(),
        migration_config_key: Some(migration_config_key.to_string()),
        scheduled_at_unix_seconds: now_unix_seconds(),
    };
    let Ok(json_line) = serde_json::to_string(&notice).map(|json| format!("{json}\n")) else {
        return;
    };

    if let Some(parent) = notice_path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        tracing::error!(
            error = %err,
            notice_path = %notice_path.display(),
            "failed to create directory for pending model migration notice"
        );
        return;
    }

    if let Err(err) = std::fs::write(&notice_path, json_line) {
        tracing::error!(
            error = %err,
            notice_path = %notice_path.display(),
            "failed to persist pending model migration notice"
        );
    }
}

pub(crate) async fn run_startup_model_migration_prompt(
    tui: &mut crate::tui::Tui,
    config: &Config,
    models_manager: &codex_core::models_manager::manager::ModelsManager,
    notice: &PendingModelMigrationNotice,
) -> Result<ModelMigrationOutcome> {
    use tokio_stream::StreamExt as _;

    let available_models = models_manager.try_list_models(config).ok();
    let copy = migration_copy_for_notice(notice, available_models.as_deref());

    let mut screen = ModelMigrationScreen::new(tui.frame_requester(), copy);
    tui.frame_requester().schedule_frame();

    let tui_events = tui.event_stream();
    tokio::pin!(tui_events);

    while let Some(event) = tui_events.next().await {
        match event {
            crate::tui::TuiEvent::Key(key_event) => {
                screen.handle_key(key_event);
                if screen.is_done() {
                    return Ok(screen.outcome());
                }
            }
            crate::tui::TuiEvent::Draw => {
                let height = tui.terminal.size()?.height;
                tui.draw(height, |frame| {
                    frame.render_widget_ref(&screen, frame.area());
                })?;
            }
            crate::tui::TuiEvent::Paste(_) => {}
        }
    }

    Ok(ModelMigrationOutcome::Accepted)
}

pub(crate) fn migration_copy_for_notice(
    notice: &PendingModelMigrationNotice,
    available_models: Option<&[ModelPreset]>,
) -> ModelMigrationCopy {
    let from_model = notice.from_model.as_str();
    let to_model = notice.to_model.as_str();

    let from_preset = available_models
        .unwrap_or_default()
        .iter()
        .find(|preset| preset.model == from_model);
    let to_preset = available_models
        .unwrap_or_default()
        .iter()
        .find(|preset| preset.model == to_model);

    let upgrade = from_preset
        .and_then(|preset| preset.upgrade.as_ref())
        .filter(|upgrade| upgrade.id == to_model);

    let can_opt_out = from_preset
        .map(|preset| preset.show_in_picker)
        .unwrap_or(true);

    migration_copy_for_models(
        from_model,
        to_model,
        upgrade.and_then(|u| u.model_link.clone()),
        upgrade.and_then(|u| u.upgrade_copy.clone()),
        to_preset
            .map(|preset| preset.display_name.clone())
            .unwrap_or_else(|| to_model.to_string()),
        to_preset
            .map(|preset| Some(preset.description.clone()))
            .unwrap_or(None),
        can_opt_out,
    )
}

const PENDING_MODEL_MIGRATION_NOTICE_FILENAME: &str = "pending_model_migration_notice.json";

fn pending_model_migration_notice_path(config: &Config) -> PathBuf {
    config
        .codex_home
        .join(PENDING_MODEL_MIGRATION_NOTICE_FILENAME)
}

fn migration_prompt_hidden(config: &Config, migration_config_key: &str) -> bool {
    match migration_config_key {
        HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG => config
            .notices
            .hide_gpt_5_1_codex_max_migration_prompt
            .unwrap_or(false),
        HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG => {
            config.notices.hide_gpt5_1_migration_prompt.unwrap_or(false)
        }
        _ => false,
    }
}

fn clear_pending_model_migration_notice(config: &Config) {
    let _ = std::fs::remove_file(pending_model_migration_notice_path(config));
}

fn now_unix_seconds() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

fn notice_expired(notice: &PendingModelMigrationNotice) -> bool {
    let Some(scheduled_at) = notice.scheduled_at_unix_seconds else {
        return false;
    };
    let Some(now) = now_unix_seconds() else {
        return false;
    };

    const WEEK_SECONDS: u64 = 7 * 24 * 60 * 60;
    now.saturating_sub(scheduled_at) > WEEK_SECONDS
}

fn should_show_model_migration_notice(
    current_model: &str,
    target_model: &str,
    available_models: &[ModelPreset],
    config: &Config,
) -> bool {
    if target_model == current_model {
        return false;
    }

    if let Some(seen_target) = config.notices.model_migrations.get(current_model)
        && seen_target == target_model
    {
        return false;
    }

    if available_models
        .iter()
        .any(|preset| preset.model == current_model && preset.upgrade.is_some())
    {
        return true;
    }

    available_models
        .iter()
        .any(|preset| preset.upgrade.as_ref().map(|u| u.id.as_str()) == Some(target_model))
}

mod prompt_ui {
    use crate::key_hint;
    use crate::render::Insets;
    use crate::render::renderable::ColumnRenderable;
    use crate::render::renderable::Renderable;
    use crate::render::renderable::RenderableExt as _;
    use crate::selection_list::selection_option_row;
    use crate::tui::FrameRequester;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyEventKind;
    use crossterm::event::KeyModifiers;
    use ratatui::prelude::Stylize as _;
    use ratatui::prelude::Widget;
    use ratatui::text::Line;
    use ratatui::text::Span;
    use ratatui::widgets::Clear;
    use ratatui::widgets::Paragraph;
    use ratatui::widgets::WidgetRef;
    use ratatui::widgets::Wrap;

    /// Outcome of the migration prompt.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) enum ModelMigrationOutcome {
        Accepted,
        Rejected,
        Exit,
    }

    #[derive(Clone)]
    pub(crate) struct ModelMigrationCopy {
        pub heading: Vec<Span<'static>>,
        pub content: Vec<Line<'static>>,
        pub can_opt_out: bool,
    }

    pub(crate) struct ModelMigrationScreen {
        request_frame: FrameRequester,
        copy: ModelMigrationCopy,
        done: bool,
        outcome: ModelMigrationOutcome,
        highlighted_option: MigrationMenuOption,
    }

    pub(crate) fn migration_copy_for_models(
        current_model: &str,
        target_model: &str,
        model_link: Option<String>,
        migration_copy: Option<String>,
        target_display_name: String,
        target_description: Option<String>,
        can_opt_out: bool,
    ) -> ModelMigrationCopy {
        let heading_text = Span::from(format!(
            "Codex just got an upgrade. Introducing {target_display_name}."
        ))
        .bold();
        let description_line: Line<'static>;
        if let Some(migration_copy) = &migration_copy {
            description_line = Line::from(migration_copy.clone());
        } else {
            description_line = target_description
                .filter(|desc| !desc.is_empty())
                .map(Line::from)
                .unwrap_or_else(|| {
                    Line::from(format!(
                        "{target_display_name} is recommended for better performance and reliability."
                    ))
                });
        }

        let mut content = vec![];
        if migration_copy.is_none() {
            content.push(Line::from(format!(
                "We recommend switching from {current_model} to {target_model}."
            )));
            content.push(Line::from(""));
        }

        if let Some(model_link) = model_link {
            content.push(Line::from(vec![
                format!("{description_line} Learn more about {target_display_name} at ").into(),
                model_link.cyan().underlined(),
            ]));
            content.push(Line::from(""));
        } else {
            content.push(description_line);
            content.push(Line::from(""));
        }

        if can_opt_out {
            content.push(Line::from(format!(
                "You can continue using {current_model} if you prefer."
            )));
        } else {
            content.push(Line::from("Press enter to continue".dim()));
        }

        ModelMigrationCopy {
            heading: vec![heading_text],
            content,
            can_opt_out,
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum MigrationMenuOption {
        TryNewModel,
        UseExistingModel,
    }

    impl MigrationMenuOption {
        fn all() -> [Self; 2] {
            [Self::TryNewModel, Self::UseExistingModel]
        }

        fn label(self) -> &'static str {
            match self {
                Self::TryNewModel => "Try new model",
                Self::UseExistingModel => "Use existing model",
            }
        }
    }

    impl ModelMigrationScreen {
        pub(crate) fn new(request_frame: FrameRequester, copy: ModelMigrationCopy) -> Self {
            Self {
                request_frame,
                copy,
                done: false,
                outcome: ModelMigrationOutcome::Accepted,
                highlighted_option: MigrationMenuOption::TryNewModel,
            }
        }

        fn finish_with(&mut self, outcome: ModelMigrationOutcome) {
            self.outcome = outcome;
            self.done = true;
            self.request_frame.schedule_frame();
        }

        fn accept(&mut self) {
            self.finish_with(ModelMigrationOutcome::Accepted);
        }

        fn reject(&mut self) {
            self.finish_with(ModelMigrationOutcome::Rejected);
        }

        fn exit(&mut self) {
            self.finish_with(ModelMigrationOutcome::Exit);
        }

        fn confirm_selection(&mut self) {
            if self.copy.can_opt_out {
                match self.highlighted_option {
                    MigrationMenuOption::TryNewModel => self.accept(),
                    MigrationMenuOption::UseExistingModel => self.reject(),
                }
            } else {
                self.accept();
            }
        }

        fn highlight_option(&mut self, option: MigrationMenuOption) {
            if self.highlighted_option != option {
                self.highlighted_option = option;
                self.request_frame.schedule_frame();
            }
        }

        pub(crate) fn handle_key(&mut self, key_event: KeyEvent) {
            if key_event.kind == KeyEventKind::Release {
                return;
            }

            if is_ctrl_exit_combo(key_event) {
                self.exit();
                return;
            }

            if self.copy.can_opt_out {
                self.handle_menu_key(key_event.code);
            } else if matches!(key_event.code, KeyCode::Esc | KeyCode::Enter) {
                self.accept();
            }
        }

        pub(crate) fn is_done(&self) -> bool {
            self.done
        }

        pub(crate) fn outcome(&self) -> ModelMigrationOutcome {
            self.outcome
        }
    }

    impl WidgetRef for &ModelMigrationScreen {
        fn render_ref(&self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
            Clear.render(area, buf);

            let mut column = ColumnRenderable::new();
            column.push("");
            column.push(self.heading_line());
            column.push(Line::from(""));
            self.render_content(&mut column);
            if self.copy.can_opt_out {
                self.render_menu(&mut column);
            }

            column.render(area, buf);
        }
    }

    impl ModelMigrationScreen {
        fn handle_menu_key(&mut self, code: KeyCode) {
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.highlight_option(MigrationMenuOption::TryNewModel);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.highlight_option(MigrationMenuOption::UseExistingModel);
                }
                KeyCode::Char('1') => {
                    self.highlight_option(MigrationMenuOption::TryNewModel);
                    self.accept();
                }
                KeyCode::Char('2') => {
                    self.highlight_option(MigrationMenuOption::UseExistingModel);
                    self.reject();
                }
                KeyCode::Enter | KeyCode::Esc => self.confirm_selection(),
                _ => {}
            }
        }

        fn heading_line(&self) -> Line<'static> {
            let mut heading = vec![Span::raw("> ")];
            heading.extend(self.copy.heading.iter().cloned());
            Line::from(heading)
        }

        fn render_content(&self, column: &mut ColumnRenderable) {
            self.render_lines(&self.copy.content, column);
        }

        fn render_lines(&self, lines: &[Line<'static>], column: &mut ColumnRenderable) {
            for line in lines {
                column.push(
                    Paragraph::new(line.clone())
                        .wrap(Wrap { trim: false })
                        .inset(Insets::tlbr(0, 2, 0, 0)),
                );
            }
        }

        fn render_menu(&self, column: &mut ColumnRenderable) {
            column.push(Line::from(""));
            column.push(
                Paragraph::new("Choose how you'd like Codex to proceed.")
                    .wrap(Wrap { trim: false })
                    .inset(Insets::tlbr(0, 2, 0, 0)),
            );
            column.push(Line::from(""));

            for (idx, option) in MigrationMenuOption::all().into_iter().enumerate() {
                column.push(selection_option_row(
                    idx,
                    option.label().to_string(),
                    self.highlighted_option == option,
                ));
            }

            column.push(Line::from(""));
            column.push(
                Line::from(vec![
                    "Use ".dim(),
                    key_hint::plain(KeyCode::Up).into(),
                    "/".dim(),
                    key_hint::plain(KeyCode::Down).into(),
                    " to move, press ".dim(),
                    key_hint::plain(KeyCode::Enter).into(),
                    " to confirm".dim(),
                ])
                .inset(Insets::tlbr(0, 2, 0, 0)),
            );
        }
    }

    fn is_ctrl_exit_combo(key_event: KeyEvent) -> bool {
        key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('d'))
    }
}

#[cfg(test)]
mod tests {
    use super::prompt_ui::ModelMigrationOutcome;
    use super::prompt_ui::ModelMigrationScreen;
    use super::prompt_ui::migration_copy_for_models;
    use crate::custom_terminal::Terminal;
    use crate::test_backend::VT100Backend;
    use crate::tui::FrameRequester;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use insta::assert_snapshot;
    use ratatui::layout::Rect;

    #[test]
    fn prompt_snapshot() {
        let width: u16 = 60;
        let height: u16 = 28;
        let backend = VT100Backend::new(width, height);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, width, height));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5.1-codex-mini",
                "gpt-5.1-codex-max",
                None,
                Some(
                    "Upgrade to gpt-5.2-codex for the latest and greatest agentic coding model."
                        .to_string(),
                ),
                "gpt-5.1-codex-max".to_string(),
                Some("Codex-optimized flagship for deep and fast reasoning.".to_string()),
                true,
            ),
        );

        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");

        assert_snapshot!("model_migration_prompt", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_family() {
        let backend = VT100Backend::new(65, 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 65, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5",
                "gpt-5.1",
                Some("https://www.codex.com/models/gpt-5.1".to_string()),
                None,
                "gpt-5.1".to_string(),
                Some("Broad world knowledge with strong general reasoning.".to_string()),
                false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_family", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex() {
        let backend = VT100Backend::new(60, 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5-codex",
                "gpt-5.1-codex-max",
                Some("https://www.codex.com/models/gpt-5.1-codex-max".to_string()),
                None,
                "gpt-5.1-codex-max".to_string(),
                Some("Codex-optimized flagship for deep and fast reasoning.".to_string()),
                false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex", terminal.backend());
    }

    #[test]
    fn prompt_snapshot_gpt5_codex_mini() {
        let backend = VT100Backend::new(60, 22);
        let mut terminal = Terminal::with_options(backend).expect("terminal");
        terminal.set_viewport_area(Rect::new(0, 0, 60, 22));

        let screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-5-codex-mini",
                "gpt-5.1-codex-mini",
                Some("https://www.codex.com/models/gpt-5.1-codex-mini".to_string()),
                None,
                "gpt-5.1-codex-mini".to_string(),
                Some("Optimized for codex. Cheaper, faster, but less capable.".to_string()),
                false,
            ),
        );
        {
            let mut frame = terminal.get_frame();
            frame.render_widget_ref(&screen, frame.area());
        }
        terminal.flush().expect("flush");
        assert_snapshot!("model_migration_prompt_gpt5_codex_mini", terminal.backend());
    }

    #[test]
    fn escape_key_accepts_prompt() {
        let mut screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                Some("https://www.codex.com/models/gpt-new".to_string()),
                None,
                "gpt-new".to_string(),
                Some("Latest recommended model for better performance.".to_string()),
                true,
            ),
        );

        // Simulate pressing Escape
        screen.handle_key(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert!(screen.is_done());
        // Esc should not be treated as Exit – it accepts like Enter.
        assert!(matches!(screen.outcome(), ModelMigrationOutcome::Accepted));
    }

    #[test]
    fn selecting_use_existing_model_rejects_upgrade() {
        let mut screen = ModelMigrationScreen::new(
            FrameRequester::test_dummy(),
            migration_copy_for_models(
                "gpt-old",
                "gpt-new",
                Some("https://www.codex.com/models/gpt-new".to_string()),
                None,
                "gpt-new".to_string(),
                Some("Latest recommended model for better performance.".to_string()),
                true,
            ),
        );

        screen.handle_key(KeyEvent::new(
            KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        ));
        screen.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));

        assert!(screen.is_done());
        assert!(matches!(screen.outcome(), ModelMigrationOutcome::Rejected));
    }
}
