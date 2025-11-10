use std::fs;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_execpolicy2::PolicyParser;
use codex_execpolicy2::load_default_policy;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut policy_path: Option<String> = None;

    while let Some(arg) = args.next() {
        if arg == "--policy" || arg == "-p" {
            let path = args
                .next()
                .context("expected a policy path after --policy/-p")?;
            policy_path = Some(path);
            continue;
        }
        // First non-flag argument is the subcommand.
        let subcommand = arg;
        return run_subcommand(subcommand, policy_path, args.collect());
    }

    print_usage();
    bail!("missing subcommand")
}

fn run_subcommand(
    subcommand: String,
    policy_path: Option<String>,
    args: Vec<String>,
) -> Result<()> {
    match subcommand.as_str() {
        "check" => cmd_check(policy_path, args),
        _ => {
            print_usage();
            bail!("unknown subcommand: {subcommand}")
        }
    }
}

fn cmd_check(policy_path: Option<String>, args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        bail!("usage: codex-execpolicy2 check <command tokens...>");
    }
    let policy = load_policy(policy_path)?;

    match policy.evaluate(&args) {
        Some(eval) => {
            let json = serde_json::to_string_pretty(&eval)?;
            println!("{json}");
        }
        None => {
            println!("no match");
        }
    }
    Ok(())
}

fn load_policy(policy_path: Option<String>) -> Result<codex_execpolicy2::Policy> {
    if let Some(path) = policy_path {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read policy at {}", Path::new(&path).display()))?;
        let parser = PolicyParser::new(&path, &content);
        return Ok(parser.parse()?);
    }
    Ok(load_default_policy()?)
}

fn print_usage() {
    eprintln!(
        "usage:
  codex-execpolicy2 [--policy path] check <command tokens...>"
    );
}
