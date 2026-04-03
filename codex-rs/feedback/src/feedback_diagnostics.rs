use std::collections::HashMap;

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

impl FeedbackDiagnostics {
    pub fn new(diagnostics: Vec<FeedbackDiagnostic>) -> Self {
        Self { diagnostics }
    }

    pub fn collect_from_env() -> Self {
        Self::collect_from_pairs(std::env::vars())
    }

    fn collect_from_pairs<I, K, V>(pairs: I) -> Self
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
                let value = env.get(*key)?;
                Some(format!("{key} = {value}"))
            })
            .collect::<Vec<_>>();
        if !proxy_details.is_empty() {
            diagnostics.push(FeedbackDiagnostic {
                headline: "Proxy environment variables are set and may affect connectivity."
                    .to_string(),
                details: proxy_details,
            });
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
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::FeedbackDiagnostic;
    use super::FeedbackDiagnostics;

    #[test]
    fn collect_from_pairs_reports_raw_values_and_attachment() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([
            (
                "HTTPS_PROXY",
                "https://user:password@secure-proxy.example.com:443?secret=1",
            ),
            ("http_proxy", "proxy.example.com:8080"),
            ("all_proxy", "socks5h://all-proxy.example.com:1080"),
        ]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![FeedbackDiagnostic {
                    headline: "Proxy environment variables are set and may affect connectivity."
                        .to_string(),
                    details: vec![
                        "http_proxy = proxy.example.com:8080".to_string(),
                        "HTTPS_PROXY = https://user:password@secure-proxy.example.com:443?secret=1"
                            .to_string(),
                        "all_proxy = socks5h://all-proxy.example.com:1080".to_string(),
                    ],
                },],
            }
        );

        assert_eq!(
            diagnostics.attachment_text(),
            Some(
            "Connectivity diagnostics\n\n- Proxy environment variables are set and may affect connectivity.\n  - http_proxy = proxy.example.com:8080\n  - HTTPS_PROXY = https://user:password@secure-proxy.example.com:443?secret=1\n  - all_proxy = socks5h://all-proxy.example.com:1080"
                .to_string()
            )
        );
    }

    #[test]
    fn collect_from_pairs_ignores_absent_values() {
        let diagnostics = FeedbackDiagnostics::collect_from_pairs(Vec::<(String, String)>::new());
        assert_eq!(diagnostics, FeedbackDiagnostics::default());
        assert_eq!(diagnostics.attachment_text(), None);
    }

    #[test]
    fn collect_from_pairs_preserves_whitespace_and_empty_values() {
        let diagnostics =
            FeedbackDiagnostics::collect_from_pairs([("HTTP_PROXY", "  proxy with spaces  ")]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![FeedbackDiagnostic {
                    headline: "Proxy environment variables are set and may affect connectivity."
                        .to_string(),
                    details: vec!["HTTP_PROXY =   proxy with spaces  ".to_string()],
                },],
            }
        );
    }

    #[test]
    fn collect_from_pairs_reports_values_verbatim() {
        let proxy_value = "not a valid proxy";
        let diagnostics = FeedbackDiagnostics::collect_from_pairs([("HTTP_PROXY", proxy_value)]);

        assert_eq!(
            diagnostics,
            FeedbackDiagnostics {
                diagnostics: vec![FeedbackDiagnostic {
                    headline: "Proxy environment variables are set and may affect connectivity."
                        .to_string(),
                    details: vec!["HTTP_PROXY = not a valid proxy".to_string()],
                },],
            }
        );
    }
}
