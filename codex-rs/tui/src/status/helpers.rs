use crate::exec_command::relativize_to_home;
use crate::legacy_core::config::Config;
use crate::status::StatusAccountDisplay;
use crate::text_formatting;
use chrono::DateTime;
use chrono::Local;
use chrono::Locale as ChronoLocale;
use codex_protocol::account::PlanType;
use codex_utils_absolute_path::AbsolutePathBuf;
use pure_rust_locales::locale_match;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

fn normalize_agents_display_path(path: &Path) -> String {
    dunce::simplified(path).display().to_string()
}

pub(crate) fn compose_model_display(
    model_name: &str,
    entries: &[(&str, String)],
) -> (String, Vec<String>) {
    let mut details: Vec<String> = Vec::new();
    if let Some((_, effort)) = entries.iter().find(|(k, _)| *k == "reasoning effort") {
        details.push(format!("reasoning {}", effort.to_ascii_lowercase()));
    }
    if let Some((_, summary)) = entries.iter().find(|(k, _)| *k == "reasoning summaries") {
        let summary = summary.trim();
        if summary.eq_ignore_ascii_case("none") || summary.eq_ignore_ascii_case("off") {
            details.push("summaries off".to_string());
        } else if !summary.is_empty() {
            details.push(format!("summaries {}", summary.to_ascii_lowercase()));
        }
    }

    (model_name.to_string(), details)
}

pub(crate) fn compose_agents_summary(config: &Config, paths: &[AbsolutePathBuf]) -> String {
    let mut rels: Vec<String> = Vec::new();

    for p in paths {
        let p = p.as_path();
        let file_name = p
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let display = if let Some(parent) = p.parent() {
            if parent == config.cwd.as_path() {
                file_name.clone()
            } else {
                let mut cur = config.cwd.as_path();
                let mut ups = 0usize;
                let mut reached = false;
                while let Some(c) = cur.parent() {
                    if cur == parent {
                        reached = true;
                        break;
                    }
                    cur = c;
                    ups += 1;
                }
                if reached {
                    let up = format!("..{}", std::path::MAIN_SEPARATOR);
                    format!("{}{}", up.repeat(ups), file_name)
                } else if let Ok(stripped) = p.strip_prefix(&config.cwd) {
                    normalize_agents_display_path(stripped)
                } else {
                    normalize_agents_display_path(p)
                }
            }
        } else {
            normalize_agents_display_path(p)
        };
        rels.push(display);
    }

    if rels.is_empty() {
        "<none>".to_string()
    } else {
        rels.join(", ")
    }
}

pub(crate) fn compose_account_display(
    account_display: Option<&StatusAccountDisplay>,
) -> Option<StatusAccountDisplay> {
    account_display.cloned()
}

pub(crate) fn plan_type_display_name(plan_type: PlanType) -> String {
    if plan_type.is_team_like() {
        "Business".to_string()
    } else if plan_type.is_business_like() {
        "Enterprise".to_string()
    } else if plan_type == PlanType::ProLite {
        "Pro Lite".to_string()
    } else {
        title_case(format!("{plan_type:?}").as_str())
    }
}

pub(crate) fn format_tokens_compact(value: i64) -> String {
    let value = value.max(0);
    if value == 0 {
        return "0".to_string();
    }
    if value < 1_000 {
        return value.to_string();
    }

    let value_f64 = value as f64;
    let (scaled, suffix) = if value >= 1_000_000_000_000 {
        (value_f64 / 1_000_000_000_000.0, "T")
    } else if value >= 1_000_000_000 {
        (value_f64 / 1_000_000_000.0, "B")
    } else if value >= 1_000_000 {
        (value_f64 / 1_000_000.0, "M")
    } else {
        (value_f64 / 1_000.0, "K")
    };

    let decimals = if scaled < 10.0 {
        2
    } else if scaled < 100.0 {
        1
    } else {
        0
    };

    let mut formatted = format!("{scaled:.decimals$}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }

    format!("{formatted}{suffix}")
}

pub(crate) fn format_directory_display(directory: &Path, max_width: Option<usize>) -> String {
    let formatted = if let Some(rel) = relativize_to_home(directory) {
        if rel.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~{}{}", std::path::MAIN_SEPARATOR, rel.display())
        }
    } else {
        directory.display().to_string()
    };

    if let Some(max_width) = max_width {
        if max_width == 0 {
            return String::new();
        }
        if UnicodeWidthStr::width(formatted.as_str()) > max_width {
            return text_formatting::center_truncate_path(&formatted, max_width);
        }
    }

    formatted
}

pub(crate) fn format_reset_timestamp(dt: DateTime<Local>, captured_at: DateTime<Local>) -> String {
    let time = system_chrono_locale()
        .map(|locale| time_without_seconds_for_locale(dt, locale))
        .unwrap_or_else(|| dt.format("%H:%M").to_string());
    if dt.date_naive() == captured_at.date_naive() {
        time
    } else {
        format!("{time} on {}", dt.format("%-d %b"))
    }
}

#[cfg(not(test))]
fn system_chrono_locale() -> Option<ChronoLocale> {
    #[cfg(unix)]
    {
        if let Some(locale) = chrono_locale_from_time_env(|key| std::env::var(key).ok()) {
            return Some(locale);
        }
    }

    sys_locale::get_locale()
        .as_deref()
        .and_then(parse_chrono_locale)
}

#[cfg(test)]
fn system_chrono_locale() -> Option<ChronoLocale> {
    Some(ChronoLocale::POSIX)
}

fn parse_chrono_locale(locale: &str) -> Option<ChronoLocale> {
    let locale = locale
        .split(['.', '@'])
        .next()
        .unwrap_or(locale)
        .replace('-', "_");
    match locale.as_str() {
        "" => None,
        "C" | "POSIX" => Some(ChronoLocale::POSIX),
        _ => locale.parse().ok(),
    }
}

fn time_without_seconds_for_locale(dt: DateTime<Local>, locale: ChronoLocale) -> String {
    dt.format_localized(&time_format_without_seconds(locale), locale)
        .to_string()
}

fn time_format_without_seconds(locale: ChronoLocale) -> String {
    strip_seconds_from_time_format(locale_match!(locale => LC_TIME::T_FMT))
}

fn strip_seconds_from_time_format(format: &str) -> String {
    let mut format = format.replace("%T", "%H:%M");
    for token in ["%OS", "%S"] {
        let Some(seconds_start) = format.find(token) else {
            continue;
        };

        let mut remove_start = seconds_start;
        if let Some((separator_start, _)) = format[..seconds_start]
            .char_indices()
            .next_back()
            .filter(|(_, ch)| matches!(ch, ':' | '.' | '፡'))
        {
            remove_start = separator_start;
        }

        let mut remove_end = seconds_start + token.len();
        if remove_start == seconds_start {
            while let Some(ch) = format[remove_end..].chars().next() {
                if ch.is_whitespace() || ch == '%' || ch.is_ascii_punctuation() {
                    break;
                }
                remove_end += ch.len_utf8();
            }
            while let Some((whitespace_start, ch)) =
                format[..remove_start].char_indices().next_back()
                && ch.is_whitespace()
            {
                remove_start = whitespace_start;
            }
        }

        format.replace_range(remove_start..remove_end, "");
        break;
    }

    format
}

#[cfg(any(test, unix))]
fn chrono_locale_from_time_env(
    mut env: impl FnMut(&str) -> Option<String>,
) -> Option<ChronoLocale> {
    ["LC_ALL", "LC_TIME", "LANG"]
        .into_iter()
        .find_map(|key| env(key).filter(|locale| !locale.is_empty()))
        .and_then(|locale| parse_chrono_locale(&locale))
}

fn title_case(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let rest = chars.as_str().to_ascii_lowercase();
    first.to_uppercase().collect::<String>() + &rest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_core::DEFAULT_AGENTS_MD_FILENAME;
    use crate::legacy_core::LOCAL_AGENTS_MD_FILENAME;
    use crate::legacy_core::config::ConfigBuilder;
    use chrono::TimeZone;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    async fn test_config(codex_home: &TempDir, cwd: &TempDir) -> Config {
        ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .fallback_cwd(Some(cwd.path().to_path_buf()))
            .build()
            .await
            .expect("load config")
    }

    #[test]
    fn plan_type_display_name_remaps_display_labels() {
        let cases = [
            (PlanType::Free, "Free"),
            (PlanType::Go, "Go"),
            (PlanType::Plus, "Plus"),
            (PlanType::Pro, "Pro"),
            (PlanType::ProLite, "Pro Lite"),
            (PlanType::Team, "Business"),
            (PlanType::SelfServeBusinessUsageBased, "Business"),
            (PlanType::Business, "Enterprise"),
            (PlanType::EnterpriseCbpUsageBased, "Enterprise"),
            (PlanType::Enterprise, "Enterprise"),
            (PlanType::Edu, "Edu"),
            (PlanType::Unknown, "Unknown"),
        ];

        for (plan_type, expected) in cases {
            assert_eq!(plan_type_display_name(plan_type), expected);
        }
    }

    #[test]
    fn strip_seconds_from_time_format_keeps_locale_time_compact() {
        let cases = [
            ("%T", "%H:%M"),
            ("%H:%M:%S", "%H:%M"),
            ("%I:%M:%S %p", "%I:%M %p"),
            ("%H.%M.%S", "%H.%M"),
            ("%H時%M分%S秒", "%H時%M分"),
            ("%H시 %M분 %S초", "%H시 %M분"),
            ("%R", "%R"),
        ];

        for (format, expected) in cases {
            assert_eq!(strip_seconds_from_time_format(format), expected.to_string());
        }
    }

    #[test]
    fn parse_chrono_locale_accepts_system_locale_tags() {
        assert_eq!(parse_chrono_locale("en-US"), Some(ChronoLocale::en_US));
        assert_eq!(
            parse_chrono_locale("en_US.UTF-8"),
            Some(ChronoLocale::en_US)
        );
        assert_eq!(parse_chrono_locale("nl-NL"), Some(ChronoLocale::nl_NL));
        assert_eq!(parse_chrono_locale("C"), Some(ChronoLocale::POSIX));
    }

    #[test]
    fn time_locale_env_uses_lc_time_before_lang() {
        let locale = chrono_locale_from_time_env(|key| match key {
            "LC_TIME" => Some("nl_NL.UTF-8".to_string()),
            "LANG" => Some("en_US.UTF-8".to_string()),
            _ => None,
        });

        assert_eq!(locale, Some(ChronoLocale::nl_NL));
    }

    #[test]
    fn time_locale_env_uses_lc_all_before_lc_time() {
        let locale = chrono_locale_from_time_env(|key| match key {
            "LC_ALL" => Some("en_US.UTF-8".to_string()),
            "LC_TIME" => Some("nl_NL.UTF-8".to_string()),
            _ => None,
        });

        assert_eq!(locale, Some(ChronoLocale::en_US));
    }

    #[test]
    fn locale_time_uses_locale_clock_conventions() {
        let dt = Local
            .with_ymd_and_hms(2024, 1, 2, 15, 4, 27)
            .single()
            .expect("timestamp");

        assert_eq!(
            time_without_seconds_for_locale(dt, ChronoLocale::en_US),
            "03:04 PM"
        );
        assert_eq!(
            time_without_seconds_for_locale(dt, ChronoLocale::nl_NL),
            "15:04"
        );
        assert_eq!(
            time_without_seconds_for_locale(dt, ChronoLocale::fi_FI),
            "15.04"
        );
    }

    #[tokio::test]
    async fn compose_agents_summary_includes_global_agents_path() {
        let codex_home = TempDir::new().expect("temp codex home");
        let cwd = TempDir::new().expect("temp cwd");
        let global_agents_path = codex_home.path().join(DEFAULT_AGENTS_MD_FILENAME);
        let config = test_config(&codex_home, &cwd).await;

        assert_eq!(
            compose_agents_summary(&config, &[global_agents_path.abs()]),
            format_directory_display(&global_agents_path, /*max_width*/ None)
        );
    }

    #[tokio::test]
    async fn compose_agents_summary_names_global_agents_override() {
        let codex_home = TempDir::new().expect("temp codex home");
        let cwd = TempDir::new().expect("temp cwd");
        let override_path = codex_home.path().join(LOCAL_AGENTS_MD_FILENAME);
        let config = test_config(&codex_home, &cwd).await;

        assert_eq!(
            compose_agents_summary(&config, &[override_path.abs()]),
            format_directory_display(&override_path, /*max_width*/ None)
        );
    }

    #[tokio::test]
    async fn compose_agents_summary_orders_global_before_project_agents() {
        let codex_home = TempDir::new().expect("temp codex home");
        let cwd = TempDir::new().expect("temp cwd");
        let global_agents_path = codex_home.path().join(DEFAULT_AGENTS_MD_FILENAME);
        let project_agents_path = cwd.path().join(DEFAULT_AGENTS_MD_FILENAME);
        let config = test_config(&codex_home, &cwd).await;

        let summary = compose_agents_summary(
            &config,
            &[
                global_agents_path.clone().abs(),
                project_agents_path.clone().abs(),
            ],
        );
        let mut paths = summary.split(", ");
        assert_eq!(
            paths.next(),
            Some(format_directory_display(&global_agents_path, /*max_width*/ None).as_str())
        );
        let project_path = paths.next().expect("project agents path");
        assert!(project_path.ends_with(DEFAULT_AGENTS_MD_FILENAME));
        assert_eq!(paths.next(), None);
    }
}
