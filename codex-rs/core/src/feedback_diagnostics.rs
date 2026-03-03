use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use tempfile::Builder;
use tempfile::TempDir;
use url::Url;

const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const OPENAI_BASE_URL_ENV_VAR: &str = "OPENAI_BASE_URL";
pub const FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME: &str = "codex-connectivity-diagnostics.txt";
const PROXY_ENV_VARS: &[&str] = &[
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedbackDiagnostics {
    diagnostics: Vec<FeedbackDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedbackDiagnostic {
    pub headline: String,
    pub details: Vec<String>,
}

pub struct FeedbackDiagnosticsAttachment {
    _dir: TempDir,
    path: PathBuf,
}

impl FeedbackDiagnosticsAttachment {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl FeedbackDiagnostics {
    pub fn collect_from_env() -> Self {
        Self::collect_from_pairs(std::env::vars())
    }

    pub fn collect_from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let env = pairs
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect::<HashMap<_, _>>();
        let mut diagnostics = Vec::new();

        let proxy_details = PROXY_ENV_VARS
            .iter()
            .filter_map(|key| {
                let value = env.get(*key)?.trim();
                if value.is_empty() {
                    return None;
                }

                let detail = match sanitize_proxy_value(value) {
                    Some(sanitized) => format!("{key} = {sanitized}"),
                    None => format!("{key} = invalid value"),
                };
                Some(detail)
            })
            .collect::<Vec<_>>();
        if !proxy_details.is_empty() {
            diagnostics.push(FeedbackDiagnostic {
                headline: "Proxy environment variables are set and may affect connectivity."
                    .to_string(),
                details: proxy_details,
            });
        }

        if let Some(value) = env.get(OPENAI_BASE_URL_ENV_VAR).map(String::as_str) {
            let trimmed = value.trim();
            if !trimmed.is_empty() && trimmed.trim_end_matches('/') != DEFAULT_OPENAI_BASE_URL {
                let detail = match sanitize_url_for_display(trimmed) {
                    Some(sanitized) => format!("{OPENAI_BASE_URL_ENV_VAR} = {sanitized}"),
                    None => format!("{OPENAI_BASE_URL_ENV_VAR} = invalid value"),
                };
                diagnostics.push(FeedbackDiagnostic {
                    headline: "OPENAI_BASE_URL is set and may affect connectivity.".to_string(),
                    details: vec![detail],
                });
            }
        }

        Self { diagnostics }
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn diagnostics(&self) -> &[FeedbackDiagnostic] {
        &self.diagnostics
    }

    pub fn attachment_text(&self) -> Option<String> {
        if self.diagnostics.is_empty() {
            return None;
        }

        let mut lines = vec!["Connectivity diagnostics".to_string(), String::new()];
        for diagnostic in &self.diagnostics {
            lines.push(format!("- {}", diagnostic.headline));
            lines.extend(
                diagnostic
                    .details
                    .iter()
                    .map(|detail| format!("  - {detail}")),
            );
        }

        Some(lines.join("\n"))
    }

    pub fn write_temp_attachment(&self) -> io::Result<Option<FeedbackDiagnosticsAttachment>> {
        let Some(text) = self.attachment_text() else {
            return Ok(None);
        };

        let dir = Builder::new()
            .prefix("codex-connectivity-diagnostics-")
            .tempdir()?;
        let path = dir.path().join(FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME);
        fs::write(&path, text)?;

        Ok(Some(FeedbackDiagnosticsAttachment { _dir: dir, path }))
    }
}

pub fn sanitize_url_for_display(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let Ok(mut url) = Url::parse(trimmed) else {
        return None;
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string().trim_end_matches('/').to_string()).filter(|value| !value.is_empty())
}

fn sanitize_proxy_value(raw: &str) -> Option<String> {
    if raw.contains("://") {
        return sanitize_url_for_display(raw);
    }

    sanitize_url_for_display(&format!("http://{raw}"))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::ffi::OsStr;
    use std::fs;

    use super::FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME;
    use super::FeedbackDiagnostic;
    use super::FeedbackDiagnostics;
    use super::sanitize_url_for_display;

    #[test]
    fn collect_from_pairs_returns_empty_when_no_diagnostics_are_present() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs(Vec::<(String, String)>::new());

        assert_eq!(diagnostics, FeedbackDiagnostics::default());
        assert_eq!(diagnostics.attachment_text(), None);
    }

    #[test]
    fn collect_from_pairs_reports_proxy_env_vars_in_fixed_order() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([
            ("HTTPS_PROXY", "https://secure-proxy.example.com:443"),
            ("HTTP_PROXY", "proxy.example.com:8080"),
            ("ALL_PROXY", "socks5h://all-proxy.example.com:1080"),
        ]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![FeedbackDiagnostic {
                    headline: "Proxy environment variables are set and may affect connectivity."
                        .to_string(),
                    details: vec![
                        "HTTP_PROXY = http://proxy.example.com:8080".to_string(),
                        "HTTPS_PROXY = https://secure-proxy.example.com".to_string(),
                        "ALL_PROXY = socks5h://all-proxy.example.com:1080".to_string(),
                    ],
                }],
            }
        );
    }

    #[test]
    fn collect_from_pairs_reports_invalid_proxy_values_without_echoing_them() {
        let diagnostics =
            FeedbackDiagnostics::collect_from_pairs([("HTTP_PROXY", "not a valid\nproxy")]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![FeedbackDiagnostic {
                    headline: "Proxy environment variables are set and may affect connectivity."
                        .to_string(),
                    details: vec!["HTTP_PROXY = invalid value".to_string()],
                }],
            }
        );
    }

    #[test]
    fn collect_from_pairs_reports_non_default_openai_base_url() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([(
            "OPENAI_BASE_URL",
            "https://example.com/v1",
        )]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![FeedbackDiagnostic {
                    headline: "OPENAI_BASE_URL is set and may affect connectivity.".to_string(),
                    details: vec!["OPENAI_BASE_URL = https://example.com/v1".to_string()],
                }],
            }
        );
    }

    #[test]
    fn collect_from_pairs_ignores_default_openai_base_url() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([(
            "OPENAI_BASE_URL",
            "https://api.openai.com/v1/",
        )]);

        assert_eq!(diagnostics, FeedbackDiagnostics::default());
    }

    #[test]
    fn collect_from_pairs_reports_invalid_openai_base_url_without_echoing_it() {
        let diagnostics =
            FeedbackDiagnostics::collect_from_pairs([("OPENAI_BASE_URL", "not a valid\nurl")]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![FeedbackDiagnostic {
                    headline: "OPENAI_BASE_URL is set and may affect connectivity.".to_string(),
                    details: vec!["OPENAI_BASE_URL = invalid value".to_string()],
                }],
            }
        );
    }

    #[test]
    fn sanitize_url_for_display_strips_credentials_query_and_fragment() {
        let sanitized = sanitize_url_for_display(
            "https://user:password@example.com:8443/v1?token=secret#fragment",
        );

        assert_eq!(sanitized, Some("https://example.com:8443/v1".to_string()));
    }

    #[test]
    fn write_temp_attachment_persists_sanitized_text() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([
            (
                "HTTP_PROXY",
                "https://user:password@proxy.example.com:8443?secret=1",
            ),
            ("OPENAI_BASE_URL", "https://example.com/v1?token=secret"),
        ]);

        let attachment = diagnostics
            .write_temp_attachment()
            .expect("attachment should be written")
            .expect("attachment should be present");
        let contents =
            fs::read_to_string(attachment.path()).expect("attachment should be readable");

        assert_eq!(
            attachment.path().file_name(),
            Some(OsStr::new(FEEDBACK_DIAGNOSTICS_ATTACHMENT_FILENAME))
        );
        assert_eq!(
            contents,
            "Connectivity diagnostics\n\n- Proxy environment variables are set and may affect connectivity.\n  - HTTP_PROXY = https://proxy.example.com:8443\n- OPENAI_BASE_URL is set and may affect connectivity.\n  - OPENAI_BASE_URL = https://example.com/v1"
                .to_string()
        );
    }
}
