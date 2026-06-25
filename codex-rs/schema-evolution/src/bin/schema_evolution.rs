use anyhow::Context;
use anyhow::Result;
use codex_schema_evolution::ApiSchema;
use codex_schema_evolution::KnownBreakageLog;
use codex_schema_evolution::check_request_narrowing;
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LintInput {
    before: Value,
    after: Value,
    before_known_breakages: String,
    after_known_breakages: String,
}

fn main() -> Result<ExitCode> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read lint input from stdin")?;
    let input: LintInput =
        serde_json::from_str(&input).context("parse lint input JSON from stdin")?;
    let before = ApiSchema::parse(&input.before).context("parse before schema")?;
    let after = ApiSchema::parse(&input.after).context("parse after schema")?;
    let before_log = KnownBreakageLog::parse(&input.before_known_breakages, "before")?;
    let after_log = KnownBreakageLog::parse(&input.after_known_breakages, "after")?;
    let breakages = check_request_narrowing(&before, &after, &before_log, &after_log)?;

    if breakages.is_empty() {
        println!("request schema does not narrow the baseline");
        Ok(ExitCode::SUCCESS)
    } else {
        println!(
            "{} request schema breakage(s) recorded in the known-breakage log",
            breakages.len()
        );
        Ok(ExitCode::FAILURE)
    }
}
