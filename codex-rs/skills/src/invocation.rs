use std::path::Path;

use codex_protocol::parse_command::ParsedCommand;
use codex_shell_command::parse_command::parse_command_impl;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

use crate::SkillMetadata;

/// A skill document read or script execution identified in a shell command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImplicitSkillAccess {
    Document(PathUri),
    Script(PathUri),
}

/// Provides the indexed skill lookups used to recognize implicit invocations.
pub trait ImplicitSkillLookup {
    fn implicit_skill_for_scripts_dir(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata>;

    fn implicit_skill_for_doc_path(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata>;
}

pub fn detect_implicit_skill_invocation_for_command(
    outcome: &impl ImplicitSkillLookup,
    command: &str,
    workdir: &AbsolutePathBuf,
) -> Option<SkillMetadata> {
    let workdir = canonicalize_if_exists(workdir);
    let tokens = tokenize_command(command);

    if let Some(candidate) = detect_skill_script_run(outcome, tokens.as_slice(), &workdir) {
        return Some(candidate);
    }

    detect_skill_doc_read(outcome, tokens.as_slice(), &workdir)
        .or_else(|| detect_powershell_skill_doc_read(outcome, command, &workdir))
}

/// Resolves statically recognizable skill accesses without consulting the host filesystem.
pub fn implicit_skill_accesses_for_command(
    command: &str,
    workdir: &PathUri,
) -> Vec<ImplicitSkillAccess> {
    // Normalize Windows paths and recognize PowerShell reads using existing cat parsing.
    let tokens = if workdir.infer_path_convention() == Some(PathConvention::Windows) {
        let mut tokens = tokenize_command(&command.replace('\\', "/"));

        if let Some(executable) = tokens.first_mut()
            && matches!(
                executable.to_ascii_lowercase().as_str(),
                "get-content" | "gc" | "type"
            )
        {
            *executable = "cat".to_owned();
        }

        tokens
    } else {
        tokenize_command(command)
    };
    let mut accesses = Vec::new();
    if let Some(script) = script_run_token(&tokens)
        && let Ok(path) = workdir.join(script)
    {
        accesses.push(ImplicitSkillAccess::Script(path));
    }

    for parsed in parse_command_impl(&tokens) {
        if let ParsedCommand::Read { path, .. } = parsed
            && let Some(path) = path.to_str()
            && let Ok(path) = workdir.join(path)
        {
            accesses.push(ImplicitSkillAccess::Document(path));
        }
    }

    accesses
}

fn tokenize_command(command: &str) -> Vec<String> {
    shlex::split(command)
        .unwrap_or_else(|| command.split_whitespace().map(str::to_string).collect())
}

fn script_run_token(tokens: &[String]) -> Option<&str> {
    const RUNNERS: [&str; 10] = [
        "python", "python3", "bash", "zsh", "sh", "node", "deno", "ruby", "perl", "pwsh",
    ];
    const SCRIPT_EXTENSIONS: [&str; 7] = [".py", ".sh", ".js", ".ts", ".rb", ".pl", ".ps1"];

    let runner_token = tokens.first()?;
    let runner = command_basename(runner_token).to_ascii_lowercase();
    let runner = runner.strip_suffix(".exe").unwrap_or(&runner);
    if !RUNNERS.contains(&runner) {
        return None;
    }

    let mut script_token = None;
    for token in tokens.iter().skip(1) {
        if token == "--" || token.starts_with('-') {
            continue;
        }
        script_token = Some(token.as_str());
        break;
    }
    let script_token = script_token?;
    if SCRIPT_EXTENSIONS
        .iter()
        .any(|extension| script_token.to_ascii_lowercase().ends_with(extension))
    {
        return Some(script_token);
    }

    None
}

fn detect_skill_script_run(
    outcome: &impl ImplicitSkillLookup,
    tokens: &[String],
    workdir: &AbsolutePathBuf,
) -> Option<SkillMetadata> {
    let script_token = script_run_token(tokens)?;
    let script_path = Path::new(script_token);
    let script_path = canonicalize_if_exists(&workdir.join(script_path));

    for path in script_path.ancestors() {
        if let Some(candidate) = outcome.implicit_skill_for_scripts_dir(&path) {
            return Some(candidate.clone());
        }
    }

    None
}

fn detect_skill_doc_read(
    outcome: &impl ImplicitSkillLookup,
    tokens: &[String],
    workdir: &AbsolutePathBuf,
) -> Option<SkillMetadata> {
    for command in parse_command_impl(tokens) {
        if let ParsedCommand::Read { path, .. } = command {
            let candidate_path = canonicalize_if_exists(&workdir.join(path.as_path()));
            if let Some(candidate) = outcome.implicit_skill_for_doc_path(&candidate_path) {
                return Some(candidate.clone());
            }
        }
    }

    None
}

fn detect_powershell_skill_doc_read(
    outcome: &impl ImplicitSkillLookup,
    command: &str,
    workdir: &AbsolutePathBuf,
) -> Option<SkillMetadata> {
    let path = powershell_get_content_path(command)?;
    let candidate_path = canonicalize_if_exists(&workdir.join(Path::new(path)));
    outcome
        .implicit_skill_for_doc_path(&candidate_path)
        .cloned()
}

fn powershell_get_content_path(command: &str) -> Option<&str> {
    let mut arguments = command.trim().strip_prefix("Get-Content ")?;
    if let Some(remaining_arguments) = arguments.strip_prefix("-Raw ") {
        arguments = remaining_arguments;
    }

    let (path, trailing) = if let Some(quoted_path) = arguments.strip_prefix('"') {
        let closing_quote = quoted_path.find('"')?;
        (
            &quoted_path[..closing_quote],
            &quoted_path[closing_quote + 1..],
        )
    } else {
        let path_end = arguments
            .char_indices()
            .find_map(|(index, character)| character.is_whitespace().then_some(index))
            .unwrap_or(arguments.len());
        (&arguments[..path_end], &arguments[path_end..])
    };

    if path.is_empty() || path.starts_with('-') || !trailing.trim().is_empty() {
        return None;
    }
    Some(path)
}

fn command_basename(command: &str) -> String {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
        .to_string()
}

fn canonicalize_if_exists(path: &AbsolutePathBuf) -> AbsolutePathBuf {
    path.canonicalize().unwrap_or_else(|_| path.clone())
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
