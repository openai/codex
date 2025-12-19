use super::new_status_output;
use super::rate_limit_snapshot_display;
use crate::history_cell::HistoryCell;
use chrono::Duration as ChronoDuration;
use chrono::TimeZone;
use chrono::Utc;
use codex_core::AuthManager;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::models_manager::manager::ModelsManager;
use codex_core::models_manager::model_family::ModelFamily;
use codex_core::protocol::CreditsSnapshot;
use codex_core::protocol::RateLimitSnapshot;
use codex_core::protocol::RateLimitWindow;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::TokenUsage;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ReasoningEffort;
use insta::assert_snapshot;
use ratatui::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;

async fn test_config(temp_home: &TempDir) -> Config {
    ConfigBuilder::default()
        .codex_home(temp_home.path().to_path_buf())
        .build()
        .await
        .expect("load config")
}

fn test_auth_manager(config: &Config) -> AuthManager {
    AuthManager::new(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    )
}

fn test_model_family(model_slug: &str, config: &Config) -> ModelFamily {
    ModelsManager::construct_model_family_offline(model_slug, config)
}

fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}

fn sanitize_directory(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| {
            if let (Some(dir_pos), Some(pipe_idx)) = (line.find("Directory: "), line.rfind('│')) {
                let prefix = &line[..dir_pos + "Directory: ".len()];
                let suffix = &line[pipe_idx..];
                let content_width = pipe_idx.saturating_sub(dir_pos + "Directory: ".len());
                let replacement = "[[workspace]]";
                let mut rebuilt = prefix.to_string();
                rebuilt.push_str(replacement);
                if content_width > replacement.len() {
                    rebuilt.push_str(&" ".repeat(content_width - replacement.len()));
                }
                rebuilt.push_str(suffix);
                rebuilt
            } else {
                line
            }
        })
        .collect()
}

fn reset_at_from(captured_at: &chrono::DateTime<chrono::Local>, seconds: i64) -> i64 {
    (*captured_at + ChronoDuration::seconds(seconds))
        .with_timezone(&Utc)
        .timestamp()
}

#[tokio::test]
async fn status_snapshot_includes_reasoning_details() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.model_provider_id = "openai".to_string();
    config.model_reasoning_effort = Some(ReasoningEffort::High);
    config.model_reasoning_summary = ReasoningSummary::Detailed;
    config
        .sandbox_policy
        .set(SandboxPolicy::WorkspaceWrite {
            writable_roots: Vec::new(),
            network_access: false,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        })
        .expect("set sandbox policy");

    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 1_200,
        cached_input_tokens: 200,
        output_tokens: 900,
        reasoning_output_tokens: 150,
        total_tokens: 2_250,
    };

    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent: 72.5,
            window_minutes: Some(300),
            resets_at: Some(reset_at_from(&captured_at, 600)),
        }),
        secondary: Some(RateLimitWindow {
            used_percent: 45.0,
            window_minutes: Some(10080),
            resets_at: Some(reset_at_from(&captured_at, 1_200)),
        }),
        credits: None,
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);

    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(80));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");
    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_snapshot_includes_monthly_limit() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.model_provider_id = "openai".to_string();
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 800,
        cached_input_tokens: 0,
        output_tokens: 400,
        reasoning_output_tokens: 0,
        total_tokens: 1_200,
    };

    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 5, 6, 7, 8, 9)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent: 12.0,
            window_minutes: Some(43_200),
            resets_at: Some(reset_at_from(&captured_at, 86_400)),
        }),
        secondary: None,
        credits: None,
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(80));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");
    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_snapshot_shows_unlimited_credits() {
    let temp_home = TempDir::new().expect("temp home");
    let config = test_config(&temp_home).await;
    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage::default();
    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 2, 3, 4, 5, 6)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: None,
        secondary: None,
        credits: Some(CreditsSnapshot {
            has_credits: true,
            unlimited: true,
            balance: None,
        }),
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);
    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let rendered = render_lines(&composite.display_lines(120));
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("Credits:") && line.contains("Unlimited")),
        "expected Credits: Unlimited line, got {rendered:?}"
    );
}

#[tokio::test]
async fn status_snapshot_shows_positive_credits() {
    let temp_home = TempDir::new().expect("temp home");
    let config = test_config(&temp_home).await;
    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage::default();
    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 3, 4, 5, 6, 7)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: None,
        secondary: None,
        credits: Some(CreditsSnapshot {
            has_credits: true,
            unlimited: false,
            balance: Some("12.5".to_string()),
        }),
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);
    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let rendered = render_lines(&composite.display_lines(120));
    assert!(
        rendered
            .iter()
            .any(|line| line.contains("Credits:") && line.contains("13 credits")),
        "expected Credits line with rounded credits, got {rendered:?}"
    );
}

#[tokio::test]
async fn status_snapshot_hides_zero_credits() {
    let temp_home = TempDir::new().expect("temp home");
    let config = test_config(&temp_home).await;
    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage::default();
    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 4, 5, 6, 7, 8)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: None,
        secondary: None,
        credits: Some(CreditsSnapshot {
            has_credits: true,
            unlimited: false,
            balance: Some("0".to_string()),
        }),
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);
    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let rendered = render_lines(&composite.display_lines(120));
    assert!(
        rendered.iter().all(|line| !line.contains("Credits:")),
        "expected no Credits line, got {rendered:?}"
    );
}

#[tokio::test]
async fn status_snapshot_hides_when_has_no_credits_flag() {
    let temp_home = TempDir::new().expect("temp home");
    let config = test_config(&temp_home).await;
    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage::default();
    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 5, 6, 7, 8, 9)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: None,
        secondary: None,
        credits: Some(CreditsSnapshot {
            has_credits: false,
            unlimited: true,
            balance: None,
        }),
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);
    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let rendered = render_lines(&composite.display_lines(120));
    assert!(
        rendered.iter().all(|line| !line.contains("Credits:")),
        "expected no Credits line when has_credits is false, got {rendered:?}"
    );
}

#[tokio::test]
async fn status_card_token_usage_excludes_cached_tokens() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 1_200,
        cached_input_tokens: 200,
        output_tokens: 900,
        reasoning_output_tokens: 0,
        total_tokens: 2_100,
    };

    let now = chrono::Local
        .with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
        .single()
        .expect("timestamp");

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        None,
        None,
        now,
        &model_slug,
    );
    let rendered = render_lines(&composite.display_lines(120));

    assert!(
        rendered.iter().all(|line| !line.contains("cached")),
        "cached tokens should not be displayed, got: {rendered:?}"
    );
}

#[tokio::test]
async fn status_snapshot_truncates_in_narrow_terminal() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.model_provider_id = "openai".to_string();
    config.model_reasoning_effort = Some(ReasoningEffort::High);
    config.model_reasoning_summary = ReasoningSummary::Detailed;
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 1_200,
        cached_input_tokens: 200,
        output_tokens: 900,
        reasoning_output_tokens: 150,
        total_tokens: 2_250,
    };

    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent: 72.5,
            window_minutes: Some(300),
            resets_at: Some(reset_at_from(&captured_at, 600)),
        }),
        secondary: None,
        credits: None,
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(70));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");

    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_snapshot_shows_missing_limits_message() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 500,
        cached_input_tokens: 0,
        output_tokens: 250,
        reasoning_output_tokens: 0,
        total_tokens: 750,
    };

    let now = chrono::Local
        .with_ymd_and_hms(2024, 2, 3, 4, 5, 6)
        .single()
        .expect("timestamp");

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        None,
        None,
        now,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(80));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");
    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_snapshot_includes_credits_and_limits() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex".to_string());
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 1_500,
        cached_input_tokens: 100,
        output_tokens: 600,
        reasoning_output_tokens: 0,
        total_tokens: 2_200,
    };

    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 7, 8, 9, 10, 11)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent: 45.0,
            window_minutes: Some(300),
            resets_at: Some(reset_at_from(&captured_at, 900)),
        }),
        secondary: Some(RateLimitWindow {
            used_percent: 30.0,
            window_minutes: Some(10_080),
            resets_at: Some(reset_at_from(&captured_at, 2_700)),
        }),
        credits: Some(CreditsSnapshot {
            has_credits: true,
            unlimited: false,
            balance: Some("37.5".to_string()),
        }),
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(80));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");
    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_snapshot_shows_empty_limits_message() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 500,
        cached_input_tokens: 0,
        output_tokens: 250,
        reasoning_output_tokens: 0,
        total_tokens: 750,
    };

    let snapshot = RateLimitSnapshot {
        primary: None,
        secondary: None,
        credits: None,
        plan_type: None,
    };
    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 6, 7, 8, 9, 10)
        .single()
        .expect("timestamp");
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        captured_at,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(80));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");
    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_snapshot_shows_stale_limits_message() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex-max".to_string());
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 1_200,
        cached_input_tokens: 200,
        output_tokens: 900,
        reasoning_output_tokens: 150,
        total_tokens: 2_250,
    };

    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent: 72.5,
            window_minutes: Some(300),
            resets_at: Some(reset_at_from(&captured_at, 600)),
        }),
        secondary: Some(RateLimitWindow {
            used_percent: 40.0,
            window_minutes: Some(10_080),
            resets_at: Some(reset_at_from(&captured_at, 1_800)),
        }),
        credits: None,
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);
    let now = captured_at + ChronoDuration::minutes(20);

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        now,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(80));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");
    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_snapshot_cached_limits_hide_credits_without_flag() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model = Some("gpt-5.1-codex".to_string());
    config.cwd = PathBuf::from("/workspace/tests");

    let auth_manager = test_auth_manager(&config);
    let usage = TokenUsage {
        input_tokens: 900,
        cached_input_tokens: 200,
        output_tokens: 350,
        reasoning_output_tokens: 0,
        total_tokens: 1_450,
    };

    let captured_at = chrono::Local
        .with_ymd_and_hms(2024, 9, 10, 11, 12, 13)
        .single()
        .expect("timestamp");
    let snapshot = RateLimitSnapshot {
        primary: Some(RateLimitWindow {
            used_percent: 60.0,
            window_minutes: Some(300),
            resets_at: Some(reset_at_from(&captured_at, 1_200)),
        }),
        secondary: Some(RateLimitWindow {
            used_percent: 35.0,
            window_minutes: Some(10_080),
            resets_at: Some(reset_at_from(&captured_at, 2_400)),
        }),
        credits: Some(CreditsSnapshot {
            has_credits: false,
            unlimited: false,
            balance: Some("80".to_string()),
        }),
        plan_type: None,
    };
    let rate_display = rate_limit_snapshot_display(&snapshot, captured_at);
    let now = captured_at + ChronoDuration::minutes(20);

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &usage,
        Some(&usage),
        &None,
        Some(&rate_display),
        None,
        now,
        &model_slug,
    );
    let mut rendered_lines = render_lines(&composite.display_lines(80));
    if cfg!(windows) {
        for line in &mut rendered_lines {
            *line = line.replace('\\', "/");
        }
    }
    let sanitized = sanitize_directory(rendered_lines).join("\n");
    assert_snapshot!(sanitized);
}

#[tokio::test]
async fn status_context_window_uses_last_usage() {
    let temp_home = TempDir::new().expect("temp home");
    let mut config = test_config(&temp_home).await;
    config.model_context_window = Some(272_000);

    let auth_manager = test_auth_manager(&config);
    let total_usage = TokenUsage {
        input_tokens: 12_800,
        cached_input_tokens: 0,
        output_tokens: 879,
        reasoning_output_tokens: 0,
        total_tokens: 102_000,
    };
    let last_usage = TokenUsage {
        input_tokens: 12_800,
        cached_input_tokens: 0,
        output_tokens: 879,
        reasoning_output_tokens: 0,
        total_tokens: 13_679,
    };

    let now = chrono::Local
        .with_ymd_and_hms(2024, 6, 1, 12, 0, 0)
        .single()
        .expect("timestamp");

    let model_slug = ModelsManager::get_model_offline(config.model.as_deref());
    let model_family = test_model_family(&model_slug, &config);
    let composite = new_status_output(
        &config,
        &auth_manager,
        &model_family,
        &total_usage,
        Some(&last_usage),
        &None,
        None,
        None,
        now,
        &model_slug,
    );
    let rendered_lines = render_lines(&composite.display_lines(80));
    let context_line = rendered_lines
        .into_iter()
        .find(|line| line.contains("Context window"))
        .expect("context line");

    assert!(
        context_line.contains("13.7K used / 272K"),
        "expected context line to reflect last usage tokens, got: {context_line}"
    );
    assert!(
        !context_line.contains("102K"),
        "context line should not use total aggregated tokens, got: {context_line}"
    );
}
