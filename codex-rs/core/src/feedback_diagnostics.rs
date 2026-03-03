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
    fn collect_from_pairs_reports_sanitized_diagnostics_and_attachment() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([
            (
                "HTTPS_PROXY",
                "https://user:password@secure-proxy.example.com:443?secret=1",
            ),
            ("http_proxy", "proxy.example.com:8080"),
            ("all_proxy", "socks5h://all-proxy.example.com:1080"),
            ("OPENAI_BASE_URL", "https://example.com/v1?token=secret"),
        ]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![
                    FeedbackDiagnostic {
                        headline:
                            "Proxy environment variables are set and may affect connectivity."
                                .to_string(),
                        details: vec![
                            "http_proxy = http://proxy.example.com:8080".to_string(),
                            "HTTPS_PROXY = https://secure-proxy.example.com".to_string(),
                            "all_proxy = socks5h://all-proxy.example.com:1080".to_string(),
                        ],
                    },
                    FeedbackDiagnostic {
                        headline: "OPENAI_BASE_URL is set and may affect connectivity.".to_string(),
                        details: vec!["OPENAI_BASE_URL = https://example.com/v1".to_string()],
                    },
                ],
            }
        );

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
            "Connectivity diagnostics\n\n- Proxy environment variables are set and may affect connectivity.\n  - http_proxy = http://proxy.example.com:8080\n  - HTTPS_PROXY = https://secure-proxy.example.com\n  - all_proxy = socks5h://all-proxy.example.com:1080\n- OPENAI_BASE_URL is set and may affect connectivity.\n  - OPENAI_BASE_URL = https://example.com/v1"
                .to_string()
        );
    }

    #[test]
    fn collect_from_pairs_ignores_absent_and_default_values() {
        for diagnostics in [
            FeedbackDiagnostics::collect_from_pairs(Vec::<(String, String)>::new()),
            FeedbackDiagnostics::collect_from_pairs([(
                "OPENAI_BASE_URL",
                "https://api.openai.com/v1/",
            )]),
        ] {
            assert_eq!(diagnostics, FeedbackDiagnostics::default());
            assert_eq!(diagnostics.attachment_text(), None);
            assert!(
                diagnostics
                    .write_temp_attachment()
                    .expect("empty diagnostics should not write attachment")
                    .is_none()
            );
        }
    }

    #[test]
    fn collect_from_pairs_reports_invalid_values_without_echoing_them() {
        let invalid_proxy = "not a valid\nproxy";
        let invalid_base_url = "not a valid\nurl";
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([
            ("HTTP_PROXY", invalid_proxy),
            ("OPENAI_BASE_URL", invalid_base_url),
        ]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![
                    FeedbackDiagnostic {
                        headline:
                            "Proxy environment variables are set and may affect connectivity."
                                .to_string(),
                        details: vec!["HTTP_PROXY = invalid value".to_string()],
                    },
                    FeedbackDiagnostic {
                        headline: "OPENAI_BASE_URL is set and may affect connectivity.".to_string(),
                        details: vec!["OPENAI_BASE_URL = invalid value".to_string()],
                    },
                ],
            }
        );
        let attachment_text = diagnostics
            .attachment_text()
            .expect("invalid diagnostics should still render attachment text");
        assert!(!attachment_text.contains(invalid_proxy));
        assert!(!attachment_text.contains(invalid_base_url));
    }

    #[test]
    fn sanitize_url_for_display_strips_credentials_query_and_fragment() {
        let sanitized = sanitize_url_for_display(
            "https://user:password@example.com:8443/v1?token=secret#fragment",
        );

        assert_eq!(sanitized, Some("https://example.com:8443/v1".to_string()));
    }
}
