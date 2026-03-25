use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::skills::SkillLoadOutcome;
use crate::skills::SkillMetadata;

pub(crate) fn build_implicit_skill_path_indexes(
    skills: Vec<SkillMetadata>,
) -> (
    HashMap<PathBuf, SkillMetadata>,
    HashMap<PathBuf, SkillMetadata>,
) {
    let mut by_scripts_dir = HashMap::new();
    let mut by_skill_doc_path = HashMap::new();
    for skill in skills {
        let skill_doc_path = normalize_path(skill.path_to_skills_md.as_path());
        by_skill_doc_path.insert(skill_doc_path, skill.clone());

        if let Some(skill_dir) = skill.path_to_skills_md.parent() {
            let scripts_dir = normalize_path(&skill_dir.join("scripts"));
            by_scripts_dir.insert(scripts_dir, skill);
        }
    }

    (by_scripts_dir, by_skill_doc_path)
}

pub fn detect_implicit_skill_invocation_for_command(
    outcome: &SkillLoadOutcome,
    command: &str,
    workdir: &Path,
) -> Option<SkillMetadata> {
    let workdir = normalize_path(workdir);
    let tokens = tokenize_command(command);

    if let Some(candidate) = detect_skill_script_run(outcome, tokens.as_slice(), workdir.as_path())
    {
        return Some(candidate);
    }

    detect_skill_doc_read(outcome, tokens.as_slice(), workdir.as_path())
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
    outcome: &SkillLoadOutcome,
    tokens: &[String],
    workdir: &Path,
) -> Option<SkillMetadata> {
    let script_token = script_run_token(tokens)?;
    let script_path = Path::new(script_token);
    let script_path = if script_path.is_absolute() {
        script_path.to_path_buf()
    } else {
        workdir.join(script_path)
    };
    let script_path = normalize_path(script_path.as_path());

    for ancestor in script_path.ancestors() {
        if let Some(candidate) = outcome.implicit_skills_by_scripts_dir.get(ancestor) {
            return Some(candidate.clone());
        }
    }

    None
}

fn detect_skill_doc_read(
    outcome: &SkillLoadOutcome,
    tokens: &[String],
    workdir: &Path,
) -> Option<SkillMetadata> {
    if !command_reads_file(tokens) {
        return None;
    }

    for token in tokens.iter().skip(1) {
        if token.starts_with('-') {
            continue;
        }
        let path = Path::new(token);
        let candidate_path = if path.is_absolute() {
            normalize_path(path)
        } else {
            normalize_path(&workdir.join(path))
        };
        if let Some(candidate) = outcome.implicit_skills_by_doc_path.get(&candidate_path) {
            return Some(candidate.clone());
        }
    }

    None
}

fn command_reads_file(tokens: &[String]) -> bool {
    const READERS: [&str; 8] = ["cat", "sed", "head", "tail", "less", "more", "bat", "awk"];
    let Some(program) = tokens.first() else {
        return false;
    };
    let program = command_basename(program).to_ascii_lowercase();
    READERS.contains(&program.as_str())
}

fn command_basename(command: &str) -> String {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
        .to_string()
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
#[path = "invocation_utils_tests.rs"]
mod tests;
