#![deny(clippy::unwrap_used, clippy::expect_used)]

mod app;
mod cli;
pub mod env_detect;
mod new_task;
pub mod scrollable_diff;
mod ui;
pub use cli::Cli;

use base64::Engine as _;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

pub(crate) fn append_error_log(message: impl AsRef<str>) {
    let ts = Utc::now().to_rfc3339();
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("error.log")
    {
        let _ = writeln!(f, "[{ts}] {}", message.as_ref());
    }
}

// (no standalone patch summarizer needed – UI displays raw diffs)

/// Entry point for the `codex cloud` subcommand.
pub async fn run_main(_cli: Cli, _codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    // Very minimal logging setup; mirrors other crates' pattern.
    let default_level = "error";
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap_or_else(|_| EnvFilter::new(default_level)),
        )
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(std::io::stderr)
        .try_init();

    info!("Launching Cloud Tasks list UI");

    // Default to online unless explicitly configured to use mock.
    let use_mock = matches!(
        std::env::var("CODEX_CLOUD_TASKS_MODE").ok().as_deref(),
        Some("mock") | Some("MOCK")
    );

    use std::sync::Arc;
    let backend: Arc<dyn codex_cloud_tasks_client::CloudBackend> = if use_mock {
        Arc::new(codex_cloud_tasks_client::MockClient)
    } else {
        // Build an HTTP client against the configured (or default) base URL.
        let base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
            .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
        let ua = codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
        let mut http =
            codex_cloud_tasks_client::HttpClient::new(base_url.clone())?.with_user_agent(ua);
        // Log which base URL and path style we're going to use.
        let style = if base_url.contains("/backend-api") {
            "wham"
        } else {
            "codex-api"
        };
        append_error_log(format!("startup: base_url={base_url} path_style={style}"));

        // Require ChatGPT login (SWIC). Exit with a clear message if missing.
        let _token = match codex_core::config::find_codex_home()
            .ok()
            .map(|home| {
                codex_login::AuthManager::new(
                    home,
                    codex_login::AuthMode::ChatGPT,
                    "codex_cloud_tasks_tui".to_string(),
                )
            })
            .and_then(|am| am.auth())
        {
            Some(auth) => {
                // Log account context for debugging workspace selection.
                if let Some(acc) = auth.get_account_id() {
                    append_error_log(format!(
                        "auth: mode=ChatGPT account_id={acc} plan={}",
                        auth.get_plan_type()
                            .unwrap_or_else(|| "<unknown>".to_string())
                    ));
                }
                match auth.get_token().await {
                    Ok(t) if !t.is_empty() => {
                        // Attach token and ChatGPT-Account-Id header if available
                        http = http.with_bearer_token(t.clone());
                        if let Some(acc) = auth
                            .get_account_id()
                            .or_else(|| extract_chatgpt_account_id(&t))
                        {
                            append_error_log(format!("auth: set ChatGPT-Account-Id header: {acc}"));
                            http = http.with_chatgpt_account_id(acc);
                        }
                        t
                    }
                    _ => {
                        eprintln!(
                            "Not signed in. Please run 'codex login' to sign in with ChatGPT, then re-run 'codex cloud'."
                        );
                        std::process::exit(1);
                    }
                }
            }
            None => {
                eprintln!(
                    "Not signed in. Please run 'codex login' to sign in with ChatGPT, then re-run 'codex cloud'."
                );
                std::process::exit(1);
            }
        };
        Arc::new(http)
    };

    // Terminal setup
    use crossterm::ExecutableCommand;
    use crossterm::event::KeyboardEnhancementFlags;
    use crossterm::event::PopKeyboardEnhancementFlags;
    use crossterm::event::PushKeyboardEnhancementFlags;
    use crossterm::terminal::EnterAlternateScreen;
    use crossterm::terminal::LeaveAlternateScreen;
    use crossterm::terminal::disable_raw_mode;
    use crossterm::terminal::enable_raw_mode;
    use ratatui::Terminal;
    use ratatui::backend::CrosstermBackend;
    let mut stdout = std::io::stdout();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    // Enable enhanced key reporting so Shift+Enter is distinguishable from Enter.
    // Some terminals may not support these flags; ignore errors if enabling fails.
    let _ = crossterm::execute!(
        std::io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        )
    );
    let backend_ui = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend_ui)?;
    terminal.clear()?;

    // App state
    let mut app = app::App::new();
    // Initial load
    let force_internal = matches!(
        std::env::var("CODEX_CLOUD_TASKS_FORCE_INTERNAL")
            .ok()
            .as_deref(),
        Some("1") | Some("true") | Some("TRUE")
    );
    append_error_log(format!(
        "startup: wham_force_internal={} ua={}",
        force_internal,
        codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"))
    ));
    // Non-blocking initial load so the in-box spinner can animate
    app.status = "Loading tasks…".to_string();
    app.refresh_inflight = true;
    // New list generation; reset background enrichment coordination
    app.list_generation = app.list_generation.saturating_add(1);
    app.in_flight.clear();
    // reset any in-flight enrichment state

    // Event stream
    use crossterm::event::Event;
    use crossterm::event::EventStream;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEventKind;
    use crossterm::event::KeyModifiers;
    use tokio_stream::StreamExt;
    let mut events = EventStream::new();

    // Channel for non-blocking background loads
    use tokio::sync::mpsc::unbounded_channel;
    let (tx, mut rx) = unbounded_channel::<app::AppEvent>();
    // Kick off the initial load in background
    {
        let backend2 = backend.clone();
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let res = app::load_tasks(&*backend2, None).await;
            let _ = tx2.send(app::AppEvent::TasksLoaded {
                env: None,
                result: res,
            });
        });
    }
    // Fetch environment list in parallel so the header can show friendly names quickly.
    {
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
                .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
            while base_url.ends_with('/') {
                base_url.pop();
            }
            if (base_url.starts_with("https://chatgpt.com")
                || base_url.starts_with("https://chat.openai.com"))
                && !base_url.contains("/backend-api")
            {
                base_url = format!("{base_url}/backend-api");
            }
            let ua =
                codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::USER_AGENT,
                reqwest::header::HeaderValue::from_str(&ua)
                    .unwrap_or(reqwest::header::HeaderValue::from_static("codex-cli")),
            );
            if let Ok(home) = codex_core::config::find_codex_home() {
                let am = codex_login::AuthManager::new(
                    home,
                    codex_login::AuthMode::ChatGPT,
                    "codex_cloud_tasks_tui".to_string(),
                );
                if let Some(auth) = am.auth()
                    && let Ok(tok) = auth.get_token().await
                    && !tok.is_empty()
                {
                    let v = format!("Bearer {tok}");
                    if let Ok(hv) = reqwest::header::HeaderValue::from_str(&v) {
                        headers.insert(reqwest::header::AUTHORIZATION, hv);
                    }
                    if let Some(acc) = auth
                        .get_account_id()
                        .or_else(|| extract_chatgpt_account_id(&tok))
                        && let Ok(name) =
                            reqwest::header::HeaderName::from_bytes(b"ChatGPT-Account-Id")
                        && let Ok(hv) = reqwest::header::HeaderValue::from_str(&acc)
                    {
                        headers.insert(name, hv);
                    }
                }
            }
            let res = crate::env_detect::list_environments(&base_url, &headers).await;
            let _ = tx2.send(app::AppEvent::EnvironmentsLoaded(res));
        });
    }

    // Try to auto-detect a likely environment id on startup and refresh if found.
    // Do this concurrently so the initial list shows quickly; on success we refetch with filter.
    {
        let tx2 = tx.clone();
        tokio::spawn(async move {
            // Normalize base URL like envcheck.rs does
            let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
                .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
            while base_url.ends_with('/') {
                base_url.pop();
            }
            if (base_url.starts_with("https://chatgpt.com")
                || base_url.starts_with("https://chat.openai.com"))
                && !base_url.contains("/backend-api")
            {
                base_url = format!("{base_url}/backend-api");
            }

            // Build headers: UA + ChatGPT auth if available
            let ua =
                codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                reqwest::header::USER_AGENT,
                reqwest::header::HeaderValue::from_str(&ua)
                    .unwrap_or(reqwest::header::HeaderValue::from_static("codex-cli")),
            );
            if let Ok(home) = codex_core::config::find_codex_home() {
                let am = codex_login::AuthManager::new(
                    home,
                    codex_login::AuthMode::ChatGPT,
                    "codex_cloud_tasks_tui".to_string(),
                );
                if let Some(auth) = am.auth()
                    && let Ok(token) = auth.get_token().await
                    && !token.is_empty()
                {
                    if let Ok(hv) =
                        reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
                    {
                        headers.insert(reqwest::header::AUTHORIZATION, hv);
                    }
                    if let Some(account_id) = auth
                        .get_account_id()
                        .or_else(|| extract_chatgpt_account_id(&token))
                        && let Ok(name) =
                            reqwest::header::HeaderName::from_bytes(b"ChatGPT-Account-Id")
                        && let Ok(hv) = reqwest::header::HeaderValue::from_str(&account_id)
                    {
                        headers.insert(name, hv);
                    }
                }
            }

            // Run autodetect. If it fails, we keep using "All".
            let res = crate::env_detect::autodetect_environment_id(&base_url, &headers, None).await;
            let _ = tx2.send(app::AppEvent::EnvironmentAutodetected(res));
        });
    }

    // Event-driven redraws with a tiny coalescing scheduler (snappy UI, no fixed 250ms tick).
    let mut needs_redraw = true;
    use std::time::Instant;
    use tokio::time::Instant as TokioInstant;
    use tokio::time::sleep_until;
    let (frame_tx, mut frame_rx) = tokio::sync::mpsc::unbounded_channel::<Instant>();
    let (redraw_tx, mut redraw_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

    // Coalesce frame requests to the earliest deadline; emit a single redraw signal.
    tokio::spawn(async move {
        let mut next_deadline: Option<Instant> = None;
        loop {
            let target =
                next_deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(24 * 60 * 60));
            let sleeper = sleep_until(TokioInstant::from_std(target));
            tokio::pin!(sleeper);
            tokio::select! {
                recv = frame_rx.recv() => {
                    match recv {
                        Some(at) => {
                            if next_deadline.is_none_or(|cur| at < cur) {
                                next_deadline = Some(at);
                            }
                            continue; // recompute sleep target
                        }
                        None => break,
                    }
                }
                _ = &mut sleeper => {
                    if next_deadline.take().is_some() {
                        let _ = redraw_tx.send(());
                    }
                }
            }
        }
    });
    // Kick an initial draw so the UI appears immediately.
    let _ = frame_tx.send(Instant::now());

    // Render helper to centralize immediate redraws after handling events.
    let render_if_needed = |terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
                            app: &mut app::App,
                            needs_redraw: &mut bool|
     -> anyhow::Result<()> {
        if *needs_redraw {
            terminal.draw(|f| ui::draw(f, app))?;
            *needs_redraw = false;
        }
        Ok(())
    };

    let exit_code = loop {
        tokio::select! {
            // Coalesced redraw requests: spinner animation and paste-burst micro‑flush.
            Some(()) = redraw_rx.recv() => {
                // Micro‑flush pending first key held by paste‑burst.
                if let Some(page) = app.new_task.as_mut() {
                    if page.composer.flush_paste_burst_if_due() { needs_redraw = true; }
                    if page.composer.is_in_paste_burst() {
                        let _ = frame_tx.send(Instant::now() + codex_tui::ComposerInput::recommended_flush_delay());
                    }
                }
                // Advance throbber only while loading.
                if app.refresh_inflight
                    || app.details_inflight
                    || app.env_loading
                    || app.apply_preflight_inflight
                    || app.apply_inflight
                {
                    app.throbber.calc_next();
                    needs_redraw = true;
                    let _ = frame_tx.send(Instant::now() + Duration::from_millis(100));
                }
                render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
            }
            maybe_app_event = rx.recv() => {
                if let Some(ev) = maybe_app_event {
                    match ev {
                        app::AppEvent::TasksLoaded { env, result } => {
                            // Only apply results for the current filter to avoid races.
                            if env.as_deref() != app.env_filter.as_deref() {
                                append_error_log(format!(
                                    "refresh.drop: env={} current={}",
                                    env.clone().unwrap_or_else(|| "<all>".to_string()),
                                    app.env_filter.clone().unwrap_or_else(|| "<all>".to_string())
                                ));
                                continue;
                            }
                            app.refresh_inflight = false;
                            match result {
                                Ok(tasks) => {
                                    append_error_log(format!(
                                        "refresh.apply: env={} count={}",
                                        env.clone().unwrap_or_else(|| "<all>".to_string()),
                                        tasks.len()
                                    ));
                                    app.tasks = tasks;
                                    if app.selected >= app.tasks.len() { app.selected = app.tasks.len().saturating_sub(1); }
                                    app.status = "Loaded tasks".to_string();
                                }
                                Err(e) => {
                                    append_error_log(format!("refresh load_tasks failed: {e}"));
                                    app.status = format!("Failed to load tasks: {e}");
                                }
                            }
                            needs_redraw = true;
                            let _ = frame_tx.send(Instant::now());
                        }
                        app::AppEvent::NewTaskSubmitted(result) => {
                            match result {
                                Ok(created) => {
                                    append_error_log(format!("new-task: created id={}", created.id.0));
                                    app.status = format!("Submitted as {}", created.id.0);
                                    app.new_task = None;
                                    // Refresh tasks in background for current filter
                                    app.status = format!("Submitted as {} — refreshing…", created.id.0);
                                    app.refresh_inflight = true;
                                    app.list_generation = app.list_generation.saturating_add(1);
                                    needs_redraw = true;
                                    let backend2 = backend.clone();
                                    let tx2 = tx.clone();
                                    let env_sel = app.env_filter.clone();
                                    tokio::spawn(async move {
                                        let res = app::load_tasks(&*backend2, env_sel.as_deref()).await;
                                        let _ = tx2.send(app::AppEvent::TasksLoaded { env: env_sel, result: res });
                                    });
                                    let _ = frame_tx.send(Instant::now());
                                }
                                Err(msg) => {
                                    append_error_log(format!("new-task: submit failed: {msg}"));
                                    if let Some(page) = app.new_task.as_mut() { page.submitting = false; }
                                    app.status = format!("Submit failed: {msg}. See error.log for details.");
                                    needs_redraw = true;
                                    let _ = frame_tx.send(Instant::now());
                                }
                            }
                        }
                        // (removed TaskSummaryUpdated; unused in this prototype)
                        app::AppEvent::ApplyPreflightFinished { id, title, message, level, skipped, conflicts } => {
                            // Only update if modal is still open and ids match
                            if let Some(m) = app.apply_modal.as_mut()
                                && m.task_id == id
                            {
                                    m.title = title;
                                    m.result_message = Some(message);
                                    m.result_level = Some(level);
                                    m.skipped_paths = skipped;
                                    m.conflict_paths = conflicts;
                                    app.apply_preflight_inflight = false;
                                    needs_redraw = true;
                                    let _ = frame_tx.send(Instant::now());
                            }
                        }
                        app::AppEvent::EnvironmentsLoaded(result) => {
                            app.env_loading = false;
                            match result {
                                Ok(list) => {
                                    app.environments = list;
                                    app.env_error = None;
                                    app.env_last_loaded = Some(std::time::Instant::now());
                                }
                                Err(e) => {
                                    app.env_error = Some(e.to_string());
                                }
                            }
                            needs_redraw = true;
                            let _ = frame_tx.send(Instant::now());
                        }
                        app::AppEvent::EnvironmentAutodetected(result) => {
                            if let Ok(sel) = result {
                                // Only apply if user hasn't set a filter yet or it's different.
                                if app.env_filter.as_deref() != Some(sel.id.as_str()) {
                                    append_error_log(format!(
                                        "env.select: autodetected id={} label={}",
                                        sel.id,
                                        sel.label.clone().unwrap_or_else(|| "<none>".to_string())
                                    ));
                                    // Preseed environments with detected label so header can show it even before list arrives
                                    if let Some(lbl) = sel.label.clone() {
                                        let present = app.environments.iter().any(|r| r.id == sel.id);
                                        if !present {
                                            app.environments.push(app::EnvironmentRow { id: sel.id.clone(), label: Some(lbl), is_pinned: false, repo_hints: None });
                                        }
                                    }
                                    app.env_filter = Some(sel.id);
                                    app.status = "Loading tasks…".to_string();
                                    app.refresh_inflight = true;
                                    app.list_generation = app.list_generation.saturating_add(1);
                                    app.in_flight.clear();
                            // reset spinner state
                                    needs_redraw = true;
                                    let backend2 = backend.clone();
                                    let tx2 = tx.clone();
                                    let env_sel = app.env_filter.clone();
                                    tokio::spawn(async move {
                                        let res = app::load_tasks(&*backend2, env_sel.as_deref()).await;
                                        let _ = tx2.send(app::AppEvent::TasksLoaded { env: env_sel, result: res });
                                    });
                                    // Proactively fetch environments to resolve a friendly name for the header.
                                    app.env_loading = true;
                                    let tx3 = tx.clone();
                                    tokio::spawn(async move {
                                        // Build headers (UA + ChatGPT token + account id) like elsewhere
                                        let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL").unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
                                        while base_url.ends_with('/') { base_url.pop(); }
                                        if (base_url.starts_with("https://chatgpt.com") || base_url.starts_with("https://chat.openai.com")) && !base_url.contains("/backend-api") {
                                            base_url = format!("{base_url}/backend-api");
                                        }
                                        let ua = codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
                                        let mut headers = reqwest::header::HeaderMap::new();
                                        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_str(&ua).unwrap_or(reqwest::header::HeaderValue::from_static("codex-cli")));
                                        if let Ok(home) = codex_core::config::find_codex_home() {
                                            let am = codex_login::AuthManager::new(
                                                home,
                                                codex_login::AuthMode::ChatGPT,
                                                "codex_cloud_tasks_tui".to_string(),
                                            );
                                            if let Some(auth) = am.auth()
                                                && let Ok(tok) = auth.get_token().await
                                                && !tok.is_empty()
                                            {
                                                let v = format!("Bearer {tok}");
                                                if let Ok(hv) = reqwest::header::HeaderValue::from_str(&v) { headers.insert(reqwest::header::AUTHORIZATION, hv); }
                                                if let Some(acc) = auth.get_account_id().or_else(|| extract_chatgpt_account_id(&tok))
                                                    && let Ok(name) = reqwest::header::HeaderName::from_bytes(b"ChatGPT-Account-Id")
                                                    && let Ok(hv) = reqwest::header::HeaderValue::from_str(&acc) {
                                                    headers.insert(name, hv);
                                                }
                                            }
                                        }
                                        let res = crate::env_detect::list_environments(&base_url, &headers).await;
                                        let _ = tx3.send(app::AppEvent::EnvironmentsLoaded(res));
                                    });
                                    let _ = frame_tx.send(Instant::now());
                                }
                            }
                            // on Err, silently continue with All
                        }
                        app::AppEvent::DetailsDiffLoaded { id, title, diff } => {
                            // Only update if the overlay still corresponds to this id.
                                        if let Some(ov) = &app.diff_overlay && ov.task_id != id { continue; }
                            let mut sd = crate::scrollable_diff::ScrollableDiff::new();
                            let diff_lines: Vec<String> = diff.lines().map(|s| s.to_string()).collect();
                            sd.set_content(diff_lines.clone());
                            app.diff_overlay = Some(app::DiffOverlay{ title, task_id: id, sd, can_apply: true, diff_lines, text_lines: Vec::new(), prompt: None, current_view: app::DetailView::Diff });
                            app.details_inflight = false;
                            app.status.clear();
                            needs_redraw = true;
                        }
                        app::AppEvent::DetailsMessagesLoaded { id, title, messages, prompt } => {
                                        if let Some(ov) = &app.diff_overlay && ov.task_id != id { continue; }
                            let conv = conversation_lines(prompt.clone(), &messages);
                            if let Some(ov) = app.diff_overlay.as_mut() {
                                ov.text_lines = conv.clone();
                                ov.prompt = prompt;
                                if !ov.can_apply {
                                    ov.sd.set_content(conv);
                                    ov.current_view = app::DetailView::Prompt;
                                }
                            } else {
                                let mut sd = crate::scrollable_diff::ScrollableDiff::new();
                                sd.set_content(conv.clone());
                                app.diff_overlay = Some(app::DiffOverlay{ title, task_id: id, sd, can_apply: false, diff_lines: Vec::new(), text_lines: conv, prompt, current_view: app::DetailView::Prompt });
                            }
                            app.details_inflight = false;
                            app.status.clear();
                            needs_redraw = true;
                        }
                        app::AppEvent::DetailsFailed { id, title, error } => {
                            if let Some(ov) = &app.diff_overlay && ov.task_id != id { continue; }
                            append_error_log(format!("details failed for {}: {error}", id.0));
                            let pretty = pretty_lines_from_error(&error);
                            let mut sd = crate::scrollable_diff::ScrollableDiff::new();
                            sd.set_content(pretty);
                            app.diff_overlay = Some(app::DiffOverlay{ title, task_id: id, sd, can_apply: false, diff_lines: Vec::new(), text_lines: Vec::new(), prompt: None, current_view: app::DetailView::Prompt });
                            app.details_inflight = false;
                            needs_redraw = true;
                        }
                        app::AppEvent::ApplyFinished { id, result } => {
                            // Only update if the modal still corresponds to this id.
                            if let Some(m) = &app.apply_modal {
                                if m.task_id != id { continue; }
                            } else {
                                continue;
                            }
                            app.apply_inflight = false;
                            match result {
                                Ok(outcome) => {
                                    app.status = outcome.message.clone();
                                    if matches!(outcome.status, codex_cloud_tasks_client::ApplyStatus::Success) {
                                        app.apply_modal = None;
                                        app.diff_overlay = None;
                                        // Refresh tasks after successful apply
                                        let backend2 = backend.clone();
                                        let tx2 = tx.clone();
                                        let env_sel = app.env_filter.clone();
                                        tokio::spawn(async move {
                                            let res = app::load_tasks(&*backend2, env_sel.as_deref()).await;
                                            let _ = tx2.send(app::AppEvent::TasksLoaded { env: env_sel, result: res });
                                        });
                                    }
                                }
                                Err(e) => {
                                    append_error_log(format!("apply_task failed for {}: {e}", id.0));
                                    app.status = format!("Apply failed: {e}");
                                }
                            }
                            needs_redraw = true;
                        }
                    }
                }
                // Render immediately after processing app events.
                render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                        // Treat Ctrl-C like pressing 'q' in the current context.
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
                        {
                            if app.env_modal.is_some() {
                                // Close environment selector if open (don’t quit composer).
                                app.env_modal = None;
                                needs_redraw = true;
                            } else if app.apply_modal.is_some() {
                                app.apply_modal = None;
                                app.status = "Apply canceled".to_string();
                                needs_redraw = true;
                            } else if app.new_task.is_some() {
                                app.new_task = None;
                                app.status = "Canceled new task".to_string();
                                needs_redraw = true;
                            } else if app.diff_overlay.is_some() {
                                app.diff_overlay = None;
                                needs_redraw = true;
                            } else {
                                break 0;
                            }
                            // Render updated state immediately before continuing to next loop iteration.
                            render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
                            // Render after New Task branch to reflect input changes immediately.
                            render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
                            continue;
                        }
                        // New Task page: Ctrl+O opens environment switcher while composing.
                        let is_ctrl_o = key.modifiers.contains(KeyModifiers::CONTROL)
                            && matches!(key.code, KeyCode::Char('o') | KeyCode::Char('O'))
                            || matches!(key.code, KeyCode::Char('\u{000F}'));
                        if is_ctrl_o && app.new_task.is_some() {
                            // Close task modal/pending apply if present before opening env modal
                            app.diff_overlay = None;
                            app.env_modal = Some(app::EnvModalState { query: String::new(), selected: 0 });
                            // Cache environments until user explicitly refreshes with 'r' inside the modal.
                            let should_fetch = app.environments.is_empty();
                            if should_fetch {
                                app.env_loading = true;
                                app.env_error = None;
                                // Ensure spinner animates while loading environments.
                                let _ = frame_tx.send(Instant::now() + Duration::from_millis(100));
                            }
                            needs_redraw = true;
                            if should_fetch {
                                let tx2 = tx.clone();
                                tokio::spawn(async move {
                                    // Build headers (UA + ChatGPT token + account id)
                                    let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL")
                                        .unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
                                    while base_url.ends_with('/') { base_url.pop(); }
                                    if (base_url.starts_with("https://chatgpt.com") || base_url.starts_with("https://chat.openai.com")) && !base_url.contains("/backend-api") {
                                        base_url = format!("{base_url}/backend-api");
                                    }
                                    let ua = codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
                                    let mut headers = reqwest::header::HeaderMap::new();
                                    headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_str(&ua).unwrap_or(reqwest::header::HeaderValue::from_static("codex-cli")));
                                    if let Ok(home) = codex_core::config::find_codex_home() {
                                        let am = codex_login::AuthManager::new(
                                            home,
                                            codex_login::AuthMode::ChatGPT,
                                            "codex_cloud_tasks_tui".to_string(),
                                        );
                                        if let Some(auth) = am.auth()
                                            && let Ok(tok) = auth.get_token().await && !tok.is_empty() {
                                                let v = format!("Bearer {tok}");
                                                if let Ok(hv) = reqwest::header::HeaderValue::from_str(&v) { headers.insert(reqwest::header::AUTHORIZATION, hv); }
                                                if let Some(acc) = auth.get_account_id().or_else(|| extract_chatgpt_account_id(&tok))
                                                    && let Ok(name) = reqwest::header::HeaderName::from_bytes(b"ChatGPT-Account-Id")
                                                    && let Ok(hv) = reqwest::header::HeaderValue::from_str(&acc) {
                                                    headers.insert(name, hv);
                                                }
                                            }
                                    }
                                    let res = crate::env_detect::list_environments(&base_url, &headers).await;
                                    let _ = tx2.send(app::AppEvent::EnvironmentsLoaded(res));
                                });
                            }
                            // Render after opening env modal to show it instantly.
                            render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
                            continue;
                        }

                        // New Task page has priority when active, unless an env modal is open.
                        if let Some(page) = app.new_task.as_mut() {
                            if app.env_modal.is_some() {
                                // Defer handling to env-modal branch below.
                            } else {
                            match key.code {
                                KeyCode::Esc => {
                                    app.new_task = None;
                                    app.status = "Canceled new task".to_string();
                                    needs_redraw = true;
                                }
                                _ => {
                                    if page.submitting {
                                        // Ignore input while submitting
                                    } else if let codex_tui::ComposerAction::Submitted(text) = page.composer.input(key) {
                                            // Submit only if we have an env id
                                            if let Some(env) = page.env_id.clone() {
                                                append_error_log(format!(
                                                    "new-task: submit env={} size={}",
                                                    env,
                                                    text.chars().count()
                                                ));
                                                page.submitting = true;
                                                app.status = "Submitting new task…".to_string();
                                                let tx2 = tx.clone();
                                                let backend2 = backend.clone();
                                                tokio::spawn(async move {
                                                    let result = codex_cloud_tasks_client::CloudBackend::create_task(&*backend2, &env, &text, "main", false).await;
                                                    let evt = match result {
                                                        Ok(ok) => app::AppEvent::NewTaskSubmitted(Ok(ok)),
                                                        Err(e) => app::AppEvent::NewTaskSubmitted(Err(format!("{e}"))),
                                                    };
                                                    let _ = tx2.send(evt);
                                                });
                                            } else {
                                                app.status = "No environment selected (press 'e' to choose)".to_string();
                                            }
                                    }
                                    needs_redraw = true;
                                    // If paste‑burst is active, schedule a micro‑flush frame.
                                    if page.composer.is_in_paste_burst() {
                                        let _ = frame_tx.send(Instant::now() + codex_tui::ComposerInput::recommended_flush_delay());
                                    }
                                    // Always schedule an immediate redraw for key edits in the composer.
                                    let _ = frame_tx.send(Instant::now());
                                    // Draw now so non-char edits (e.g., Option+Delete) reflect instantly.
                                    render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
                                }
                            }
                            continue;
                            }
                        }
                        // If a diff overlay is open, handle its keys first.
                        if app.apply_modal.is_some() {
                            // Simple apply confirmation modal: y apply, p preflight, n/Esc cancel
                            match key.code {
                                KeyCode::Char('y') => {
                                    if let Some(m) = app.apply_modal.as_ref() {
                                        // Keep modal open and animate a spinner while applying
                                        app.apply_inflight = true;
                                        app.status = format!("Applying '{}'...", m.title);
                                        needs_redraw = true;
                                        let backend2 = backend.clone();
                                        let tx2 = tx.clone();
                                        let id2 = m.task_id.clone();
                                        tokio::spawn(async move {
                                            let res = codex_cloud_tasks_client::CloudBackend::apply_task(&*backend2, id2.clone()).await;
                                            let evt = match res {
                                                Ok(outcome) => app::AppEvent::ApplyFinished { id: id2, result: Ok(outcome) },
                                                Err(e) => app::AppEvent::ApplyFinished { id: id2, result: Err(format!("{e}")) },
                                            };
                                            let _ = tx2.send(evt);
                                        });
                                    }
                                }
                                KeyCode::Char('p') => {
                                    if let Some(m) = app.apply_modal.take() {
                                        // Kick off async preflight; show spinner in modal body
                                        app.apply_preflight_inflight = true;
                                        app.apply_modal = Some(app::ApplyModalState { task_id: m.task_id.clone(), title: m.title.clone(), result_message: None, result_level: None, skipped_paths: Vec::new(), conflict_paths: Vec::new() });
                                        needs_redraw = true;
                                        let _ = frame_tx.send(Instant::now() + Duration::from_millis(100));
                                        let backend2 = backend.clone();
                                        let tx2 = tx.clone();
                                        let id2 = m.task_id.clone();
                                        let title2 = m.title.clone();
                                        tokio::spawn(async move {
                                            unsafe { std::env::set_var("CODEX_APPLY_PREFLIGHT", "1") };
                                            let out = codex_cloud_tasks_client::CloudBackend::apply_task(&*backend2, id2.clone()).await;
                                            unsafe { std::env::remove_var("CODEX_APPLY_PREFLIGHT") };
                                            let evt = match out {
                                                Ok(outcome) => {
                                                    let level = match outcome.status {
                                                        codex_cloud_tasks_client::ApplyStatus::Success => app::ApplyResultLevel::Success,
                                                        codex_cloud_tasks_client::ApplyStatus::Partial => app::ApplyResultLevel::Partial,
                                                        codex_cloud_tasks_client::ApplyStatus::Error => app::ApplyResultLevel::Error,
                                                    };
                                                    app::AppEvent::ApplyPreflightFinished { id: id2, title: title2, message: outcome.message, level, skipped: outcome.skipped_paths, conflicts: outcome.conflict_paths }
                                                }
                                                Err(e) => app::AppEvent::ApplyPreflightFinished { id: id2, title: title2, message: format!("Preflight failed: {e}"), level: app::ApplyResultLevel::Error, skipped: Vec::new(), conflicts: Vec::new() },
                                            };
                                            let _ = tx2.send(evt);
                                        });
                                    }
                                }
                                KeyCode::Esc
                                | KeyCode::Char('n')
                                | KeyCode::Char('q')
                                | KeyCode::Char('Q') => { app.apply_modal = None; app.status = "Apply canceled".to_string(); needs_redraw = true; }
                                _ => {}
                            }
                        } else if app.diff_overlay.is_some() {
                            match key.code {
                                KeyCode::Char('a') => {
                                    if let Some(ov) = &app.diff_overlay {
                                        if ov.can_apply {
                                            app.apply_modal = Some(app::ApplyModalState { task_id: ov.task_id.clone(), title: ov.title.clone(), result_message: None, result_level: None, skipped_paths: Vec::new(), conflict_paths: Vec::new() });
                                            app.apply_preflight_inflight = true;
                                            let _ = frame_tx.send(Instant::now() + Duration::from_millis(100));
                                            let backend2 = backend.clone();
                                            let tx2 = tx.clone();
                                            let id2 = ov.task_id.clone();
                                            let title2 = ov.title.clone();
                                            tokio::spawn(async move {
                                                unsafe { std::env::set_var("CODEX_APPLY_PREFLIGHT", "1") };
                                                let out = codex_cloud_tasks_client::CloudBackend::apply_task(&*backend2, id2.clone()).await;
                                                unsafe { std::env::remove_var("CODEX_APPLY_PREFLIGHT") };
                                                let evt = match out {
                                                    Ok(outcome) => {
                                                        let level = match outcome.status {
                                                            codex_cloud_tasks_client::ApplyStatus::Success => app::ApplyResultLevel::Success,
                                                            codex_cloud_tasks_client::ApplyStatus::Partial => app::ApplyResultLevel::Partial,
                                                            codex_cloud_tasks_client::ApplyStatus::Error => app::ApplyResultLevel::Error,
                                                        };
                                                        app::AppEvent::ApplyPreflightFinished { id: id2, title: title2, message: outcome.message, level, skipped: outcome.skipped_paths, conflicts: outcome.conflict_paths }
                                                    }
                                                    Err(e) => app::AppEvent::ApplyPreflightFinished { id: id2, title: title2, message: format!("Preflight failed: {e}"), level: app::ApplyResultLevel::Error, skipped: Vec::new(), conflicts: Vec::new() },
                                                };
                                                let _ = tx2.send(evt);
                                            });
                                        } else {
                                            app.status = "No diff available to apply".to_string();
                                        }
                                        needs_redraw = true;
                                    }
                                }
                                // From task modal, 'o' should close it and open the env selector
                                KeyCode::Char('o') | KeyCode::Char('O') => {
                                    app.diff_overlay = None;
                                    app.env_modal = Some(app::EnvModalState { query: String::new(), selected: 0 });
                                    // Use cached environments unless empty
                                    if app.environments.is_empty() { app.env_loading = true; app.env_error = None; }
                                    needs_redraw = true;
                                    if app.environments.is_empty() {
                                        let tx2 = tx.clone();
                                        tokio::spawn(async move {
                                            // Build headers (UA + ChatGPT token + account id)
                                            let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL").unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
                                            while base_url.ends_with('/') { base_url.pop(); }
                                            if (base_url.starts_with("https://chatgpt.com") || base_url.starts_with("https://chat.openai.com")) && !base_url.contains("/backend-api") { base_url = format!("{base_url}/backend-api"); }
                                            let ua = codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
                                            let mut headers = reqwest::header::HeaderMap::new();
                                            headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_str(&ua).unwrap_or(reqwest::header::HeaderValue::from_static("codex-cli")));
                                            if let Ok(home) = codex_core::config::find_codex_home() {
                                                let am = codex_login::AuthManager::new(
                                                    home,
                                                    codex_login::AuthMode::ChatGPT,
                                                    "codex_cloud_tasks_tui".to_string(),
                                                );
                                                if let Some(auth) = am.auth() && let Ok(tok) = auth.get_token().await && !tok.is_empty() {
                                                    let v = format!("Bearer {tok}");
                                                    if let Ok(hv) = reqwest::header::HeaderValue::from_str(&v) { headers.insert(reqwest::header::AUTHORIZATION, hv); }
                                                    if let Some(acc) = auth.get_account_id().or_else(|| extract_chatgpt_account_id(&tok))
                                                        && let Ok(name) = reqwest::header::HeaderName::from_bytes(b"ChatGPT-Account-Id")
                                                        && let Ok(hv) = reqwest::header::HeaderValue::from_str(&acc) {
                                                        headers.insert(name, hv);
                                                    }
                                                }
                                            }
                                            let res = crate::env_detect::list_environments(&base_url, &headers).await;
                                            let _ = tx2.send(app::AppEvent::EnvironmentsLoaded(res));
                                        });
                                    }
                                }
                                KeyCode::Left => {
                                    if let Some(ov) = &mut app.diff_overlay {
                                        let has_text = !ov.text_lines.is_empty() || ov.prompt.is_some();
                                        let has_diff = !ov.diff_lines.is_empty() || ov.can_apply;
                                        if has_text && has_diff {
                                            ov.current_view = app::DetailView::Prompt;
                                            let lines = if ov.text_lines.is_empty() { conversation_lines(ov.prompt.clone(), &[]) } else { ov.text_lines.clone() };
                                            ov.sd.set_content(lines);
                                            ov.sd.to_top();
                                            needs_redraw = true;
                                        }
                                    }
                                }
                                KeyCode::Right => {
                                    if let Some(ov) = &mut app.diff_overlay {
                                        let has_text = !ov.text_lines.is_empty() || ov.prompt.is_some();
                                        let has_diff = !ov.diff_lines.is_empty() || ov.can_apply;
                                        if has_text && has_diff {
                                            ov.current_view = app::DetailView::Diff;
                                            let lines = ov.diff_lines.clone();
                                            if !lines.is_empty() {
                                                ov.sd.set_content(lines);
                                            }
                                            ov.sd.to_top();
                                            needs_redraw = true;
                                        }
                                    }
                                }
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    app.diff_overlay = None;
                                    needs_redraw = true;
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if let Some(ov) = &mut app.diff_overlay { ov.sd.scroll_by(1); }
                                    needs_redraw = true;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if let Some(ov) = &mut app.diff_overlay { ov.sd.scroll_by(-1); }
                                    needs_redraw = true;
                                }
                                KeyCode::PageDown | KeyCode::Char(' ') => {
                                    if let Some(ov) = &mut app.diff_overlay { let step = ov.sd.state.viewport_h.saturating_sub(1) as i16; ov.sd.page_by(step); }
                                    needs_redraw = true;
                                }
                                KeyCode::PageUp => {
                                    if let Some(ov) = &mut app.diff_overlay { let step = ov.sd.state.viewport_h.saturating_sub(1) as i16; ov.sd.page_by(-step); }
                                    needs_redraw = true;
                                }
                                KeyCode::Home => { if let Some(ov) = &mut app.diff_overlay { ov.sd.to_top(); } needs_redraw = true; }
                                KeyCode::End  => { if let Some(ov) = &mut app.diff_overlay { ov.sd.to_bottom(); } needs_redraw = true; }
                                _ => {}
                            }
                        } else if app.env_modal.is_some() {
                            // Environment modal key handling
                            match key.code {
                                KeyCode::Esc => { app.env_modal = None; needs_redraw = true; }
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    // Trigger refresh of environments
                                    app.env_loading = true; app.env_error = None; needs_redraw = true;
                                    let _ = frame_tx.send(Instant::now() + Duration::from_millis(100));
                                    let tx2 = tx.clone();
                                    tokio::spawn(async move {
                                        // Build headers (UA + ChatGPT token + account id)
                                        let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL").unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
                                        while base_url.ends_with('/') { base_url.pop(); }
                                        if (base_url.starts_with("https://chatgpt.com") || base_url.starts_with("https://chat.openai.com")) && !base_url.contains("/backend-api") { base_url = format!("{base_url}/backend-api"); }
                                        let ua = codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
                                        let mut headers = reqwest::header::HeaderMap::new();
                                        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_str(&ua).unwrap_or(reqwest::header::HeaderValue::from_static("codex-cli")));
                                        if let Ok(home) = codex_core::config::find_codex_home() {
                                            let am = codex_login::AuthManager::new(
                                                home,
                                                codex_login::AuthMode::ChatGPT,
                                                "codex_cloud_tasks_tui".to_string(),
                                            );
                                            if let Some(auth) = am.auth()
                                                && let Ok(tok) = auth.get_token().await && !tok.is_empty() {
                                                    let v = format!("Bearer {tok}");
                                                    if let Ok(hv) = reqwest::header::HeaderValue::from_str(&v) { headers.insert(reqwest::header::AUTHORIZATION, hv); }
                                                    if let Some(acc) = auth.get_account_id().or_else(|| extract_chatgpt_account_id(&tok))
                                                        && let Ok(name) = reqwest::header::HeaderName::from_bytes(b"ChatGPT-Account-Id")
                                                            && let Ok(hv) = reqwest::header::HeaderValue::from_str(&acc) { headers.insert(name, hv); }
                                                }
                                        }
                                        let res = crate::env_detect::list_environments(&base_url, &headers).await;
                                        let _ = tx2.send(app::AppEvent::EnvironmentsLoaded(res));
                                    });
                                }
                                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT) => {
                                    if let Some(m) = app.env_modal.as_mut() { m.query.push(ch); }
                                    needs_redraw = true;
                                }
                                KeyCode::Backspace => { if let Some(m) = app.env_modal.as_mut() { m.query.pop(); } needs_redraw = true; }
                                KeyCode::Down | KeyCode::Char('j') => { if let Some(m) = app.env_modal.as_mut() { m.selected = m.selected.saturating_add(1); } needs_redraw = true; }
                                KeyCode::Up | KeyCode::Char('k') => { if let Some(m) = app.env_modal.as_mut() { m.selected = m.selected.saturating_sub(1); } needs_redraw = true; }
                                KeyCode::Home => { if let Some(m) = app.env_modal.as_mut() { m.selected = 0; } needs_redraw = true; }
                                KeyCode::End => { if let Some(m) = app.env_modal.as_mut() { m.selected = app.environments.len(); } needs_redraw = true; }
                                KeyCode::PageDown | KeyCode::Char(' ') => { if let Some(m) = app.env_modal.as_mut() { let step = 10usize; m.selected = m.selected.saturating_add(step); } needs_redraw = true; }
                                KeyCode::PageUp => { if let Some(m) = app.env_modal.as_mut() { let step = 10usize; m.selected = m.selected.saturating_sub(step); } needs_redraw = true; }
                                KeyCode::Char('n') => {
                                    if app.env_filter.is_none() {
                                        app.new_task = Some(crate::new_task::NewTaskPage::new(None));
                                    } else {
                                        app.new_task = Some(crate::new_task::NewTaskPage::new(app.env_filter.clone()));
                                    }
                                    app.status = "New Task: Enter to submit; Esc to cancel".to_string();
                                    needs_redraw = true;
                                }
                                KeyCode::Enter => {
                                    // Resolve selection over filtered set
                                    if let Some(state) = app.env_modal.take() {
                                        let q = state.query.to_lowercase();
                                        let filtered: Vec<&app::EnvironmentRow> = app.environments.iter().filter(|r| {
                                            if q.is_empty() { return true; }
                                            let mut hay = String::new();
                                            if let Some(l) = &r.label { hay.push_str(&l.to_lowercase()); hay.push(' '); }
                                            hay.push_str(&r.id.to_lowercase());
                                            if let Some(h) = &r.repo_hints { hay.push(' '); hay.push_str(&h.to_lowercase()); }
                                            hay.contains(&q)
                                        }).collect();
                                        // Keep original order (already sorted) — no need to re-sort
                                        let idx = state.selected;
                                        if idx == 0 { app.env_filter = None; append_error_log("env.select: All"); }
                                        else {
                                            let env_idx = idx.saturating_sub(1);
                                            if let Some(row) = filtered.get(env_idx) {
                                                append_error_log(format!(
                                                    "env.select: id={} label={}",
                                                    row.id,
                                                    row.label.clone().unwrap_or_else(|| "<none>".to_string())
                                                ));
                                                app.env_filter = Some(row.id.clone());
                                            }
                                        }
                                        // If New Task page is open, reflect the new selection in its header immediately.
                                        if let Some(page) = app.new_task.as_mut() {
                                            page.env_id = app.env_filter.clone();
                                        }
                                        // Trigger tasks refresh with the selected filter
                                        app.status = "Loading tasks…".to_string();
                                        app.refresh_inflight = true;
                                        app.list_generation = app.list_generation.saturating_add(1);
                                        app.in_flight.clear();
                                        // reset spinner state
                                        needs_redraw = true;
                                        let backend2 = backend.clone();
                                        let tx2 = tx.clone();
                                        let env_sel = app.env_filter.clone();
                                        tokio::spawn(async move {
                                            let res = app::load_tasks(&*backend2, env_sel.as_deref()).await;
                                            let _ = tx2.send(app::AppEvent::TasksLoaded { env: env_sel, result: res });
                                        });
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            // Base list view keys
                            match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => {
                                    break 0;
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    app.next();
                                    needs_redraw = true;
                                }
                                KeyCode::Up | KeyCode::Char('k') => {
                                    app.prev();
                                    needs_redraw = true;
                                }
                                // Ensure 'r' does not refresh tasks when the env modal is open.
                                KeyCode::Char('r') | KeyCode::Char('R') => {
                                    if app.env_modal.is_some() { break 0; }
                                    append_error_log(format!(
                                        "refresh.request: env={}",
                                        app.env_filter.clone().unwrap_or_else(|| "<all>".to_string())
                                    ));
                                    app.status = "Refreshing…".to_string();
                                    app.refresh_inflight = true;
                                    app.list_generation = app.list_generation.saturating_add(1);
                                    app.in_flight.clear();
                                        // reset spinner state
                                    needs_redraw = true;
                                    // Spawn background refresh
                                    let backend2 = backend.clone();
                                    let tx2 = tx.clone();
                                    let env_sel = app.env_filter.clone();
                                    tokio::spawn(async move {
                                        let res = app::load_tasks(&*backend2, env_sel.as_deref()).await;
                                        let _ = tx2.send(app::AppEvent::TasksLoaded { env: env_sel, result: res });
                                    });
                                }
                                KeyCode::Char('o') | KeyCode::Char('O') => {
                                    app.env_modal = Some(app::EnvModalState { query: String::new(), selected: 0 });
                                    // Cache environments until user explicitly refreshes with 'r' inside the modal.
                                    let should_fetch = app.environments.is_empty();
                                    if should_fetch { app.env_loading = true; app.env_error = None; }
                                    needs_redraw = true;
                                    if should_fetch {
                                        let tx2 = tx.clone();
                                        tokio::spawn(async move {
                                            // Build headers (UA + ChatGPT token + account id)
                                            let mut base_url = std::env::var("CODEX_CLOUD_TASKS_BASE_URL").unwrap_or_else(|_| "https://chatgpt.com/backend-api".to_string());
                                            while base_url.ends_with('/') { base_url.pop(); }
                                            if (base_url.starts_with("https://chatgpt.com") || base_url.starts_with("https://chat.openai.com")) && !base_url.contains("/backend-api") { base_url = format!("{base_url}/backend-api"); }
                                            let ua = codex_core::default_client::get_codex_user_agent(Some("codex_cloud_tasks_tui"));
                                            let mut headers = reqwest::header::HeaderMap::new();
                                            headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_str(&ua).unwrap_or(reqwest::header::HeaderValue::from_static("codex-cli")));
                                            if let Ok(home) = codex_core::config::find_codex_home() {
                                                let am = codex_login::AuthManager::new(
                                                    home,
                                                    codex_login::AuthMode::ChatGPT,
                                                    "codex_cloud_tasks_tui".to_string(),
                                                );
                                                if let Some(auth) = am.auth()
                                                    && let Ok(tok) = auth.get_token().await && !tok.is_empty() {
                                                        let v = format!("Bearer {tok}");
                                                        if let Ok(hv) = reqwest::header::HeaderValue::from_str(&v) { headers.insert(reqwest::header::AUTHORIZATION, hv); }
                                                        if let Some(acc) = auth.get_account_id().or_else(|| extract_chatgpt_account_id(&tok))
                                                            && let Ok(name) = reqwest::header::HeaderName::from_bytes(b"ChatGPT-Account-Id")
                                                                && let Ok(hv) = reqwest::header::HeaderValue::from_str(&acc) { headers.insert(name, hv); }
                                                    }
                                            }
                                            let res = crate::env_detect::list_environments(&base_url, &headers).await;
                                            let _ = tx2.send(app::AppEvent::EnvironmentsLoaded(res));
                                        });
                                    }
                                }
                                KeyCode::Char('n') => {
                                    let env_opt = app.env_filter.clone();
                                    app.new_task = Some(crate::new_task::NewTaskPage::new(env_opt));
                                    app.status = "New Task: Enter to submit; Esc to cancel".to_string();
                                    needs_redraw = true;
                                }
                                KeyCode::Enter => {
                                    if let Some(task) = app.tasks.get(app.selected).cloned() {
                                        app.status = format!("Loading details for {title}…", title = task.title);
                                        app.details_inflight = true;
                                        // Open empty overlay immediately; content arrives via events
                                        let mut sd = crate::scrollable_diff::ScrollableDiff::new();
                                        sd.set_content(Vec::new());
                                        app.diff_overlay = Some(app::DiffOverlay{ title: task.title.clone(), task_id: task.id.clone(), sd, can_apply: false, diff_lines: Vec::new(), text_lines: Vec::new(), prompt: None, current_view: app::DetailView::Prompt });
                                        needs_redraw = true;
                                        // Spawn background details load (diff first, then messages fallback)
                                        let backend2 = backend.clone();
                                        let tx2 = tx.clone();
                                        let id1 = task.id.clone();
                                        let title1 = task.title.clone();
                                        let id2 = id1.clone();
                                        let title2 = title1.clone();
                                        tokio::spawn(async move {
                                            match codex_cloud_tasks_client::CloudBackend::get_task_diff(&*backend2, id1.clone()).await {
                                                Ok(diff) => {
                                                    let _ = tx2.send(app::AppEvent::DetailsDiffLoaded { id: id1, title: title1, diff });
                                                }
                                                Err(e) => {
                                                    // Always log errors while we debug non-success states.
                                                    append_error_log(format!("get_task_diff failed for {}: {e}", id1.0));
                                                    match codex_cloud_tasks_client::CloudBackend::get_task_text(&*backend2, id1.clone()).await {
                                                        Ok(text) => {
                                                            let _ = tx2.send(app::AppEvent::DetailsMessagesLoaded { id: id1, title: title1, messages: text.messages, prompt: text.prompt });
                                                        }
                                                        Err(e2) => {
                                                            let _ = tx2.send(app::AppEvent::DetailsFailed { id: id1, title: title1, error: format!("{e2}") });
                                                        }
                                                    }
                                                }
                                            }
                                        });
                                        // Also fetch conversation text even when diff exists
                                        {
                                            let backend3 = backend.clone();
                                            let tx3 = tx.clone();
                                            let id3 = id2;
                                            let title3 = title2;
                                            tokio::spawn(async move {
                                                if let Ok(text) = codex_cloud_tasks_client::CloudBackend::get_task_text(&*backend3, id3.clone()).await {
                                                    let _ = tx3.send(app::AppEvent::DetailsMessagesLoaded { id: id3, title: title3, messages: text.messages, prompt: text.prompt });
                                                }
                                            });
                                        }
                                        // Animate spinner while details load.
                                        let _ = frame_tx.send(Instant::now() + Duration::from_millis(100));
                                    }
                                }
                                KeyCode::Char('a') => {
                                    if let Some(task) = app.tasks.get(app.selected) {
                                        match codex_cloud_tasks_client::CloudBackend::get_task_diff(&*backend, task.id.clone()).await {
                                            Ok(_) => {
                                                app.apply_modal = Some(app::ApplyModalState { task_id: task.id.clone(), title: task.title.clone(), result_message: None, result_level: None, skipped_paths: Vec::new(), conflict_paths: Vec::new() });
                                                app.apply_preflight_inflight = true;
                                                let _ = frame_tx.send(Instant::now() + Duration::from_millis(100));
                                                let backend2 = backend.clone();
                                                let tx2 = tx.clone();
                                                let id2 = task.id.clone();
                                                let title2 = task.title.clone();
                                                tokio::spawn(async move {
                                                    unsafe { std::env::set_var("CODEX_APPLY_PREFLIGHT", "1") };
                                                    let out = codex_cloud_tasks_client::CloudBackend::apply_task(&*backend2, id2.clone()).await;
                                                    unsafe { std::env::remove_var("CODEX_APPLY_PREFLIGHT") };
                                                    let evt = match out {
                                                        Ok(outcome) => {
                                                            let level = match outcome.status {
                                                                codex_cloud_tasks_client::ApplyStatus::Success => app::ApplyResultLevel::Success,
                                                                codex_cloud_tasks_client::ApplyStatus::Partial => app::ApplyResultLevel::Partial,
                                                                codex_cloud_tasks_client::ApplyStatus::Error => app::ApplyResultLevel::Error,
                                                            };
                                                            app::AppEvent::ApplyPreflightFinished { id: id2, title: title2, message: outcome.message, level, skipped: outcome.skipped_paths, conflicts: outcome.conflict_paths }
                                                        }
                                                        Err(e) => app::AppEvent::ApplyPreflightFinished { id: id2, title: title2, message: format!("Preflight failed: {e}"), level: app::ApplyResultLevel::Error, skipped: Vec::new(), conflicts: Vec::new() },
                                                    };
                                                    let _ = tx2.send(evt);
                                                });
                                            }
                                            Err(_) => {
                                                app.status = "No diff available to apply".to_string();
                                            }
                                        }
                                        needs_redraw = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        // Render after handling a key event (when not quitting).
                        render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        needs_redraw = true;
                        // Redraw immediately on resize for snappier UX.
                        render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
                    }
                    Some(Err(_)) | None => {}
                    _ => {}
                }
                // Fallback: if any other event path requested a redraw, render now.
                render_if_needed(&mut terminal, &mut app, &mut needs_redraw)?;
            }
        }
    };

    // Restore terminal
    disable_raw_mode().ok();
    terminal.show_cursor().ok();
    // Best-effort restore of keyboard enhancement flags before leaving alt screen.
    let _ = crossterm::execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    let _ = crossterm::execute!(std::io::stdout(), LeaveAlternateScreen);

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn extract_chatgpt_account_id(token: &str) -> Option<String> {
    // JWT: header.payload.signature
    let mut parts = token.split('.');
    let (_h, payload_b64, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => return None,
    };
    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&payload_bytes).ok()?;
    v.get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(|id| id.as_str())
        .map(|s| s.to_string())
}

/// Build plain-text conversation lines: a labeled user prompt followed by assistant messages.
fn conversation_lines(prompt: Option<String>, messages: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(p) = prompt {
        out.push("user:".to_string());
        for l in p.lines() {
            out.push(l.to_string());
        }
        out.push(String::new());
    }
    if !messages.is_empty() {
        out.push("assistant:".to_string());
        for (i, m) in messages.iter().enumerate() {
            for l in m.lines() {
                out.push(l.to_string());
            }
            if i + 1 < messages.len() {
                out.push(String::new());
            }
        }
    }
    if out.is_empty() {
        out.push("<no output>".to_string());
    }
    out
}

/// Convert a verbose HTTP error with embedded JSON body into concise, user-friendly lines
/// for the details overlay. Falls back to a short raw message when parsing fails.
fn pretty_lines_from_error(raw: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let is_no_diff = raw.contains("No output_diff in response.");
    let is_no_msgs = raw.contains("No assistant text messages in response.");
    if is_no_diff {
        lines.push("No diff available for this task.".to_string());
    } else if is_no_msgs {
        lines.push("No assistant messages found for this task.".to_string());
    } else {
        lines.push("Failed to load task details.".to_string());
    }

    // Try to parse the embedded JSON body: find the first '{' after " body=" and decode.
    if let Some(body_idx) = raw.find(" body=")
        && let Some(json_start_rel) = raw[body_idx..].find('{')
    {
        let json_start = body_idx + json_start_rel;
        let json_str = raw[json_start..].trim();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
            // Prefer assistant turn context.
            let turn = v
                .get("current_assistant_turn")
                .and_then(|x| x.as_object())
                .cloned()
                .or_else(|| {
                    v.get("current_diff_task_turn")
                        .and_then(|x| x.as_object())
                        .cloned()
                });
            if let Some(t) = turn {
                if let Some(err) = t.get("error").and_then(|e| e.as_object()) {
                    let code = err.get("code").and_then(|s| s.as_str()).unwrap_or("");
                    let msg = err.get("message").and_then(|s| s.as_str()).unwrap_or("");
                    if !code.is_empty() || !msg.is_empty() {
                        let summary = if code.is_empty() {
                            msg.to_string()
                        } else if msg.is_empty() {
                            code.to_string()
                        } else {
                            format!("{code}: {msg}")
                        };
                        lines.push(format!("Assistant error: {summary}"));
                    }
                }
                if let Some(status) = t.get("turn_status").and_then(|s| s.as_str()) {
                    lines.push(format!("Status: {status}"));
                }
                if let Some(text) = t
                    .get("latest_event")
                    .and_then(|e| e.get("text"))
                    .and_then(|s| s.as_str())
                    && !text.trim().is_empty()
                {
                    lines.push(format!("Latest event: {}", text.trim()));
                }
            }
        }
    }

    if lines.len() == 1 {
        // Parsing yielded nothing; include a trimmed, short raw message tail for context.
        let tail = if raw.len() > 320 {
            format!("{}…", &raw[..320])
        } else {
            raw.to_string()
        };
        lines.push(tail);
    } else if lines.len() >= 2 {
        // Add a hint to refresh when still in progress.
        if lines.iter().any(|l| l.contains("in_progress")) {
            lines.push("This task may still be running. Press 'r' to refresh.".to_string());
        }
        // Avoid an empty overlay
        lines.push(String::new());
    }
    lines
}
