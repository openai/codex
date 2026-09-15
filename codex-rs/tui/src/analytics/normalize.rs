//! Converts backend reports into display data while preserving missing values and full denominators.

use super::models::AccountAnalyticsGrouping as Grouping;
use super::models::AccountAnalyticsReport as Report;
use super::models::*;
use super::report_data::AnalyticsData;
use chrono::DateTime;
use chrono::NaiveDate;
use std::collections::BTreeMap;

/// Out-of-range legacy rows do not change attribution semantics for the requested period.
pub(super) fn has_complete_attribution(
    response: &AnalyticsData,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<bool, String> {
    response
        .data
        .iter()
        .try_fold(/*init*/ true, |complete, record| {
            let date = NaiveDate::parse_from_str(
                record.date.get(..10).unwrap_or(&record.date),
                "%Y-%m-%d",
            )
            .map_err(|_| String::from("Analytics returned an invalid date."))?;
            Ok(complete && (date < start || date > end || record.attribution.is_some()))
        })
}

pub(super) fn history(
    response: AnalyticsData,
    report: Report,
    grouping: Grouping,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Option<AccountAnalyticsHistory>, String> {
    if report == Report::Messages {
        match grouping {
            Grouping::Model | Grouping::Surface => {}
            Grouping::Feature
            | Grouping::TaskStart
            | Grouping::Speed
            | Grouping::Reasoning
            | Grouping::TokenType => {
                return Err(String::from("Unsupported message count grouping."));
            }
        }
    }
    let attributed = has_complete_attribution(&response, start, end)?;
    let series = response.series.as_ref();
    let mut days: BTreeMap<NaiveDate, (f64, BTreeMap<String, f64>)> = BTreeMap::new();
    for record in response.data {
        let date =
            NaiveDate::parse_from_str(record.date.get(..10).unwrap_or(&record.date), "%Y-%m-%d")
                .map_err(|_| String::from("Analytics returned an invalid date."))?;
        if date < start || date > end {
            continue;
        }
        let (total, values) = days.entry(date).or_default();
        match report {
            Report::Usage => {
                let daily = if attributed {
                    None
                } else {
                    match grouping {
                        Grouping::Surface => record.product_surface_usage_values,
                        Grouping::Model => record
                            .models
                            .map(|models| -> Result<_, String> {
                                let mut values = BTreeMap::new();
                                for model in models {
                                    let amount = model
                                        .credits
                                        .ok_or("Plan usage amount was not reported.")?;
                                    *values.entry(model.model).or_insert(/*default*/ 0.0) += amount;
                                }
                                Ok(values)
                            })
                            .transpose()?,
                        _ => None,
                    }
                };
                if let Some(daily) = daily {
                    for (key, amount) in daily {
                        if !amount.is_finite() || amount < 0.0 {
                            return Err(String::from("Analytics returned an invalid amount."));
                        }
                        let key = if grouping == Grouping::Surface {
                            if key.starts_with("work_") {
                                "work"
                            } else {
                                "codex"
                            }
                            .to_string()
                        } else if key.eq_ignore_ascii_case("other") {
                            "other".to_string()
                        } else {
                            key
                        };
                        *total += amount;
                        *values.entry(key).or_default() += amount;
                    }
                } else if let Some(attribution) = record.attribution.filter(|_| attributed) {
                    for entry in attribution {
                        if !entry.value.is_finite() || entry.value < 0.0 {
                            return Err(String::from("Analytics returned an invalid amount."));
                        }
                        *total += entry.value;
                        let key = match grouping {
                            Grouping::Feature => entry.thread_source,
                            Grouping::Model => entry.model,
                            Grouping::Surface => entry.surface,
                            Grouping::TaskStart => {
                                entry.turn_trigger.map(|trigger| format!("start-{trigger}"))
                            }
                            Grouping::Speed | Grouping::Reasoning | Grouping::TokenType => {
                                return Err(String::from("unsupported analytics grouping"));
                            }
                        }
                        .unwrap_or_else(|| "unknown".to_string());
                        *values.entry(key).or_default() += entry.value;
                    }
                } else {
                    return Ok(None);
                }
            }

            Report::Credits => {
                if series.is_some() {
                    let Some(credits) = record.values else {
                        return Ok(None);
                    };
                    for (key, amount) in credits {
                        *values.entry(key).or_default() += amount;
                        *total += amount;
                    }
                } else if matches!(grouping, Grouping::Model | Grouping::Speed) {
                    let Some(models) = record.models else {
                        return Ok(None);
                    };
                    for model in models {
                        let amount = model.on_demand_credits.unwrap_or(/*default*/ 0.0);
                        let key = if grouping == Grouping::Speed {
                            model.speed.unwrap_or_else(|| "unknown".to_string())
                        } else {
                            model.model
                        };
                        *values.entry(key).or_default() += amount;
                        *total += amount;
                    }
                } else if let Some(premium) = record.premium_usage_values {
                    for (surface, amount) in premium.credit_usage_credits {
                        if !amount.is_finite() {
                            return Err(String::from("Analytics returned an invalid amount."));
                        }
                        *values.entry(surface).or_default() += amount;
                        *total += amount;
                    }
                } else if let (Some(amount), Some(surface)) =
                    (record.credit_amount, record.product_surface)
                {
                    *values.entry(surface).or_default() += amount;
                    *total += amount;
                } else {
                    return Ok(None);
                }
            }
            Report::Messages => {
                let authoritative = record
                    .totals
                    .ok_or_else(|| String::from("Message count is unavailable."))?
                    .turns;
                if !authoritative.is_finite() || authoritative < 0.0 {
                    return Err(String::from("Analytics returned an invalid amount."));
                }
                let mut record_values = BTreeMap::<String, f64>::new();
                if grouping == Grouping::Model {
                    for model in record.models.unwrap_or_default() {
                        let key = if model.model.eq_ignore_ascii_case("codex-auto-review")
                            || model.model.eq_ignore_ascii_case("other")
                        {
                            "other".to_string()
                        } else {
                            model.model
                        };
                        let count = model
                            .turns
                            .ok_or_else(|| String::from("Turn count is unavailable."))?;
                        if !count.is_finite() || count < 0.0 {
                            return Err(String::from("Analytics returned an invalid amount."));
                        }
                        *record_values.entry(key).or_default() += count;
                    }
                } else {
                    for client in record.clients.unwrap_or_default() {
                        if !client.turns.is_finite() || client.turns < 0.0 {
                            return Err(String::from("Analytics returned an invalid amount."));
                        }
                        let key = match client.client_id.as_str() {
                            "CODEX_CLI" => "cli",
                            "CODEX_IDE_VSCODE" => "vscode",
                            "CODEX_WEB" => "web",
                            "CODEX_DESKTOP_APP" | "CODEX_WORK_DESKTOP" => "desktop_app",
                            "CODEX_WORK_WEB" => "work_web",
                            "CODEX_WORK_MOBILE" => "mobile",
                            "CODEX_GITHUB" => "github_code_review",
                            _ => "other",
                        };
                        *record_values.entry(key.to_string()).or_default() += client.turns;
                    }
                }
                let remainder =
                    (authoritative - record_values.values().sum::<f64>()).max(/*other*/ 0.0);
                if remainder > 0.0 {
                    *record_values.entry("other".to_string()).or_default() += remainder;
                }
                for (key, count) in record_values {
                    *values.entry(key).or_default() += count;
                }
                *total += authoritative;
            }
            Report::Plugins | Report::Skills => {
                let overviews = if report == Report::Skills {
                    record.skill_usage_overviews
                } else {
                    record.plugin_usage_overviews
                }
                .ok_or_else(|| String::from("Tool activity is unavailable."))?;
                for tool in overviews {
                    if !tool.invocation_counts.is_finite() || tool.invocation_counts < 0.0 {
                        return Err(String::from("Analytics returned an invalid amount."));
                    }
                    *values.entry(tool.display_name).or_default() += tool.invocation_counts;
                    *total += tool.invocation_counts;
                }
            }
        }
        if !total.is_finite()
            || values
                .values()
                .any(|value| !value.is_finite() || (report != Report::Credits && *value < 0.0))
        {
            return Err(String::from("Analytics returned an invalid amount."));
        }
    }
    if report == Report::Usage
        && !attributed
        && grouping == Grouping::Model
        && response.units.as_deref() != Some("credits")
    {
        let mut totals = BTreeMap::<String, f64>::new();
        for (_, values) in days.values() {
            for (key, value) in values {
                *totals.entry(key.clone()).or_default() += value;
            }
        }
        let total: f64 = totals.values().sum();
        for (_, values) in days.values_mut() {
            let mut other = 0.0;
            values.retain(|key, value| {
                let amount = totals[key];
                if key.eq_ignore_ascii_case("other") || (amount > 0.0 && amount < total * 0.01) {
                    other += *value;
                    false
                } else {
                    true
                }
            });
            if other > 0.0 {
                values.insert("other".to_string(), other);
            }
        }
    }
    if report == Report::Credits {
        for date in start.iter_days().take_while(|date| *date <= end) {
            days.entry(date).or_default();
        }
    }
    Ok(Some(AccountAnalyticsHistory {
        unit: match report {
            Report::Usage if !attributed && response.units.as_deref() == Some("credits") => {
                AccountAnalyticsUnit::Credits
            }
            Report::Usage => AccountAnalyticsUnit::RelativeUsage,
            Report::Credits => AccountAnalyticsUnit::Credits,
            Report::Messages | Report::Plugins | Report::Skills => AccountAnalyticsUnit::Count,
        },
        updated_at: response
            .data_freshness_ts
            .as_deref()
            .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
            .map(|timestamp| timestamp.timestamp()),
        data: days
            .into_iter()
            .map(|(date, (total, values))| AccountAnalyticsDay {
                date,
                total,
                values: values
                    .into_iter()
                    .map(|(key, value)| AccountAnalyticsValue {
                        label: series
                            .and_then(|series| series.iter().find(|series| series.key == key))
                            .map(|series| series.label.clone())
                            .unwrap_or_else(|| label(&key).to_string()),
                        key,
                        value,
                    })
                    .collect(),
            })
            .collect(),
    }))
}

pub(super) fn label(key: &str) -> &str {
    match key {
        "user" => "Tasks",
        "subagent" => "Subagents",
        "image_generation" => "Image generation",
        "automation" => "Automations",
        "guardian_review" => "Guardian review",
        "automated_review" => "Auto review",
        "agent_identity" => "Workspace agents",
        "memory_consolidation" => "Memory consolidation",
        "cli" => "CLI",
        "desktop_app" => "Desktop app",
        "vscode" => "VS Code",
        "web" => "Web",
        "work_web" => "Work web",
        "work_desktop" => "Work desktop",
        "work_mobile" => "Work mobile",
        "mobile" => "Mobile",
        "slack" => "Slack",
        "linear" => "Linear",
        "jetbrains" => "JetBrains",
        "sdk" => "SDK",
        "exec" => "Exec",
        "github" => "GitHub",
        "codex" => "Codex",
        "work" => "Work",
        "code_review" | "github_code_review" => "Code review",
        "start-user" | "start-composer" => "User messages",
        "start-goal" => "Goals",
        "start-composer_queue" => "Queued messages",
        "start-composer_queue_run_now" => "Queued messages run now",
        "start-automation_cron_scheduled" => "Scheduled automations",
        "start-automation_cron_run_now" => "Automations run now",
        "start-automation_heartbeat_scheduled" => "Scheduled follow-ups",
        "start-automation_heartbeat_run_now" => "Follow-ups run now",
        "start-app_tool_create_thread" => "Agent-created tasks",
        "start-app_tool_send_message" => "Agent follow-ups",
        "unknown" | "start-unknown" => "Unknown",
        "fast" => "Fast",
        "standard" => "Standard",
        "other" | "start-other" => "Other",
        _ => key,
    }
}

#[cfg(test)]
#[path = "normalize_tests.rs"]
mod tests;
