use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use codex_execpolicy_legacy::ExecCall;
use codex_execpolicy_legacy::MatchedExec;
use codex_execpolicy_legacy::Policy;
use codex_execpolicy_legacy::PolicyParser;
use codex_execpolicy_legacy::ValidExec;
use codex_execpolicy_legacy::get_default_policy;
use serde::Deserialize;
use serde::Serialize;
use serde::de;
use starlark::Error as StarlarkError;
use std::path::PathBuf;
use std::str::FromStr;

const MATCHED_BUT_WRITES_FILES_EXIT_CODE: i32 = 12;
const MIGHT_BE_SAFE_EXIT_CODE: i32 = 13;
const FORBIDDEN_EXIT_CODE: i32 = 14;

#[derive(Parser, Deserialize, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// If the command fails the policy, exit with 13, but print parseable JSON
    /// to stdout.
    #[clap(long)]
    pub require_safe: bool,

    /// Path to the policy file.
    #[clap(long, short = 'p')]
    pub policy: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Debug, Deserialize, Subcommand)]
pub enum Command {
    /// Checks the command as if the arguments were the inputs to execv(3).
    Check {
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
    },

    /// Checks the command encoded as a JSON object.
    #[clap(name = "check-json")]
    CheckJson {
        /// JSON object with "program" (str) and "args" (list[str]) fields.
        #[serde(deserialize_with = "deserialize_from_json")]
        exec: ExecArg,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecArg {
    pub program: String,

    #[serde(default)]
    pub args: Vec<String>,
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Args::parse();
    let policy = match args.policy {
        Some(policy) => {
            let policy_source = policy.to_string_lossy().to_string();
            let unparsed_policy = std::fs::read_to_string(policy)?;
            let parser = PolicyParser::new(&policy_source, &unparsed_policy);
            parser.parse()
        }
        None => get_default_policy(),
    };
    let policy = policy.map_err(StarlarkError::into_anyhow)?;

    let exec = match args.command {
        Command::Check { command } => match command.split_first() {
            Some((first, rest)) => ExecArg {
                program: first.to_string(),
                args: rest.to_vec(),
            },
            None => {
                eprintln!("no command provided");
                std::process::exit(1);
            }
        },
        Command::CheckJson { exec } => exec,
    };

    let (output, exit_code) = check_command(&policy, exec, args.require_safe);
    let json = serde_json::to_string(&output)?;
    println!("{json}");
    std::process::exit(exit_code);
}

fn check_command(
    policy: &Policy,
    ExecArg { program, args }: ExecArg,
    check: bool,
) -> (Output, i32) {
    let exec_call = ExecCall { program, args };
    match policy.check(&exec_call) {
        Ok(MatchedExec::Match { exec }) => {
            if exec.might_write_files() {
                let exit_code = if check {
                    MATCHED_BUT_WRITES_FILES_EXIT_CODE
                } else {
                    0
                };
                (Output::Match { r#match: exec }, exit_code)
            } else {
                (Output::Safe { r#match: exec }, 0)
            }
        }
        Ok(MatchedExec::Forbidden { reason, cause }) => {
            let exit_code = if check { FORBIDDEN_EXIT_CODE } else { 0 };
            (Output::Forbidden { reason, cause }, exit_code)
        }
        Err(err) => {
            let exit_code = if check { MIGHT_BE_SAFE_EXIT_CODE } else { 0 };
            (Output::Unverified { error: err }, exit_code)
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "result")]
pub enum Output {
    /// The command is verified as safe.
    #[serde(rename = "safe")]
    Safe { r#match: ValidExec },

    /// The command has matched a rule in the policy, but the caller should
    /// decide whether it is "safe" given the files it wants to write.
    #[serde(rename = "match")]
    Match { r#match: ValidExec },

    /// The user is forbidden from running the command.
    #[serde(rename = "forbidden")]
    Forbidden {
        reason: String,
        cause: codex_execpolicy_legacy::Forbidden,
    },

    /// The safety of the command could not be verified.
    #[serde(rename = "unverified")]
    Unverified {
        error: codex_execpolicy_legacy::Error,
    },
}

fn deserialize_from_json<'de, D>(deserializer: D) -> Result<ExecArg, D::Error>
where
    D: de::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let decoded = serde_json::from_str(&s)
        .map_err(|e| serde::de::Error::custom(format!("JSON parse error: {e}")))?;
    Ok(decoded)
}

impl FromStr for ExecArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s).map_err(Into::into)
    }
}
