use std::time::Duration;

use crate::sandbox_summary::summarize_sandbox_policy;
use codex_core::WireApi;
use codex_core::config::Config;

/// Build a list of key/value pairs summarizing the effective configuration.
pub fn create_config_summary_entries(config: &Config, model: &str) -> Vec<(&'static str, String)> {
    let mut entries = vec![
        ("workdir", config.cwd.display().to_string()),
        ("model", model.to_string()),
        ("provider", config.model_provider_id.clone()),
        ("approval", config.approval_policy.value().to_string()),
        ("sandbox", summarize_sandbox_policy(&config.sandbox_policy)),
    ];
    if config.model_provider.wire_api == WireApi::Responses {
        let reasoning_effort = config
            .model_reasoning_effort
            .map(|effort| effort.to_string());
        entries.push((
            "reasoning effort",
            reasoning_effort.unwrap_or_else(|| "none".to_string()),
        ));
        entries.push((
            "reasoning summaries",
            config.model_reasoning_summary.to_string(),
        ));
    }

    if config.progress.no_progress {
        entries.push(("progress", "disabled".to_string()));
    } else if let Some(interval) = config.progress.interval_seconds {
        entries.push(("progress", format!("every {}s", interval.get())));
    }

    if config.auto_continue.enabled {
        let mut details = Vec::new();
        if let Some(limit) = config.auto_continue.max_turns {
            details.push(format!("max {} turn(s)", limit.get()));
        }
        if let Some(duration) = config.auto_continue.max_duration {
            details.push(format!("max {}", format_auto_continue_duration(duration)));
        }
        let summary = if details.is_empty() {
            "enabled".to_string()
        } else {
            format!("enabled ({})", details.join(", "))
        };
        entries.push(("auto-continue", summary));
    }

    entries
}

#[cfg(feature = "elapsed")]
fn format_auto_continue_duration(duration: Duration) -> String {
    crate::elapsed::format_duration(duration)
}

#[cfg(not(feature = "elapsed"))]
fn format_auto_continue_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    if secs >= 60 {
        let mins = secs / 60;
        let rem = secs % 60;
        format!("{mins}m {rem:02}s")
    } else if secs > 0 {
        if millis == 0 {
            format!("{secs}s")
        } else {
            format!("{secs}.{millis:03}s")
        }
    } else {
        format!("{millis}ms")
    }
}
