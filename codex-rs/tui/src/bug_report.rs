
use sysinfo::System;
use url::form_urlencoded::Serializer;

#[derive(Default, Clone, Debug)]
pub struct BugReportEntry {
    pub prompt: String,
    pub reasoning: u32,
    pub tools: u32,
}

/// Generate a complete bug report URL with platform info and reproducible steps.
pub fn build_bug_report_url(entries: &[BugReportEntry], model: &str) -> String {
    let platform_str = {
        let kernel = System::kernel_version().unwrap_or_else(|| "unknown".to_string());
        format!(
            "`{}` | `{}` | `{}`",
            std::env::consts::OS,
            std::env::consts::ARCH,
            kernel
        )
    };

    let mut ser = Serializer::new(String::new());
    ser.append_pair("template", "2-bug-report.yml");
    ser.append_pair("labels", "bug");
    ser.append_pair("version", env!("CARGO_PKG_VERSION"));
    ser.append_pair("model", model);
    ser.append_pair("platform", &platform_str);

    if !entries.is_empty() {
        let mut bullets = Vec::new();
        for entry in entries {
            let code_block = format!("```\n  {}\n  ```", entry.prompt.trim());
            bullets.push(format!(
                "- {}\n  - `{} reasoning` | `{} tool`",
                code_block, entry.reasoning, entry.tools
            ));
        }
        ser.append_pair("steps", &bullets.join("\n"));
    }

    let query = ser.finish();
    format!("https://github.com/openai/codex/issues/new?{}", query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use url::Url;

    fn qs(url: &str) -> HashMap<String, String> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }

    #[test]
    fn no_entries_omits_steps() {
        let q = qs(&build_bug_report_url(&[], "test-model"));
        assert_eq!(q["template"], "2-bug-report.yml");
        assert_eq!(q["labels"], "bug");
        assert_eq!(q["model"], "test-model");
        assert!(q.contains_key("version"));
        assert!(q.contains_key("platform"));
        assert!(!q.contains_key("steps"));
    }

    #[test]
    fn multiple_entries_encoded_properly() {
        let entries = vec![
            BugReportEntry { prompt: "first".into(), reasoning: 1, tools: 2 },
            BugReportEntry { prompt: "second step".into(), reasoning: 3, tools: 4 },
        ];
        let q = qs(&build_bug_report_url(&entries, "model-x"));
        let steps = &q["steps"];
        assert!(steps.contains("first"));
        assert!(steps.contains("second step"));
        assert!(steps.contains("`1 reasoning`"));
        assert!(steps.contains("`4 tool`"));
    }
}
