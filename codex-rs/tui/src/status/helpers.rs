use crate::exec_command::relativize_to_home;
use crate::status::StatusAccountDisplay;
use crate::text_formatting;
use chrono::DateTime;
use chrono::Local;
use codex_app_server_client::legacy_core::config::Config;
use codex_app_server_client::legacy_core::discover_project_doc_paths;
use codex_exec_server::LOCAL_FS;
use codex_protocol::account::PlanType;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::io;
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

pub(crate) async fn discover_agents_summary(config: &Config) -> io::Result<String> {
    let paths = discover_project_doc_paths(config, LOCAL_FS.as_ref()).await?;
    Ok(compose_agents_summary(config, &paths))
}

pub(crate) fn compose_agents_summary(config: &Config, paths: &[AbsolutePathBuf]) -> String {
    let mut rels: Vec<String> = Vec::new();
    if let Some(path) = config.user_instructions_path.as_deref() {
        rels.push(format_directory_display(path, /*max_width*/ None));
    }

    for p in paths {
        let file_name = p
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let display = if let Some(parent) = p.parent() {
            if parent.as_path() == config.cwd.as_path() {
                file_name.clone()
            } else {
                let mut cur = config.cwd.as_path();
                let mut ups = 0usize;
                let mut reached = false;
                while let Some(c) = cur.parent() {
                    if cur == parent.as_path() {
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
    let time = dt.format("%H:%M").to_string();
    if dt.date_naive() == captured_at.date_naive() {
        time
    } else {
        format!("{time} on {}", dt.format("%-d %b"))
    }
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
    use codex_app_server_client::legacy_core::DEFAULT_PROJECT_DOC_FILENAME;
    use codex_app_server_client::legacy_core::LOCAL_PROJECT_DOC_FILENAME;
    use codex_app_server_client::legacy_core::config::ConfigBuilder;
    use pretty_assertions::assert_eq;
    use std::fs;
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

    #[tokio::test]
    async fn discover_agents_summary_includes_global_agents_path() {
        let codex_home = TempDir::new().expect("temp codex home");
        let cwd = TempDir::new().expect("temp cwd");
        let global_agents_path = codex_home.path().join(DEFAULT_PROJECT_DOC_FILENAME);
        fs::write(&global_agents_path, "global instructions").expect("write global agents");
        let config = test_config(&codex_home, &cwd).await;

        assert_eq!(
            discover_agents_summary(&config).await.expect("summary"),
            format_directory_display(&global_agents_path, /*max_width*/ None)
        );
    }

    #[tokio::test]
    async fn discover_agents_summary_names_global_agents_override() {
        let codex_home = TempDir::new().expect("temp codex home");
        let cwd = TempDir::new().expect("temp cwd");
        fs::write(
            codex_home.path().join(DEFAULT_PROJECT_DOC_FILENAME),
            "global instructions",
        )
        .expect("write global agents");
        let override_path = codex_home.path().join(LOCAL_PROJECT_DOC_FILENAME);
        fs::write(&override_path, "override instructions").expect("write global override");
        let config = test_config(&codex_home, &cwd).await;

        assert_eq!(
            discover_agents_summary(&config).await.expect("summary"),
            format_directory_display(&override_path, /*max_width*/ None)
        );
    }

    #[tokio::test]
    async fn discover_agents_summary_orders_global_before_project_agents() {
        let codex_home = TempDir::new().expect("temp codex home");
        let cwd = TempDir::new().expect("temp cwd");
        let global_agents_path = codex_home.path().join(DEFAULT_PROJECT_DOC_FILENAME);
        fs::write(&global_agents_path, "global instructions").expect("write global agents");
        fs::write(
            cwd.path().join(DEFAULT_PROJECT_DOC_FILENAME),
            "project instructions",
        )
        .expect("write project agents");
        let config = test_config(&codex_home, &cwd).await;

        let summary = discover_agents_summary(&config).await.expect("summary");
        let mut paths = summary.split(", ");
        assert_eq!(
            paths.next(),
            Some(format_directory_display(&global_agents_path, /*max_width*/ None).as_str())
        );
        let project_path = paths.next().expect("project agents path");
        assert!(project_path.ends_with(DEFAULT_PROJECT_DOC_FILENAME));
        assert_eq!(paths.next(), None);
    }
}
