use std::collections::HashMap;
use std::path::Path;

use crate::SkillLoadOutcome;
use crate::SkillMetadata;
use codex_exec_server::EnvironmentPathRef;
use codex_utils_absolute_path::AbsolutePathBuf;

pub(crate) async fn build_implicit_skill_path_indexes(
    skills: Vec<SkillMetadata>,
) -> (
    HashMap<EnvironmentPathRef, SkillMetadata>,
    HashMap<EnvironmentPathRef, SkillMetadata>,
) {
    let mut by_scripts_dir = HashMap::new();
    let mut by_skill_doc_path = HashMap::new();
    for skill in skills {
        let skill_doc_path = canonicalize_if_exists(&skill.source_path).await;
        by_skill_doc_path.insert(skill_doc_path, skill.clone());

        if let Ok(Some(skill_dir)) = skill.source_path.parent().await
            && let Ok(scripts_dir) = skill_dir.join(Path::new("scripts")).await
        {
            let scripts_dir = canonicalize_if_exists(&scripts_dir).await;
            by_scripts_dir.insert(scripts_dir, skill);
        }
    }

    (by_scripts_dir, by_skill_doc_path)
}

pub async fn detect_implicit_skill_invocation_for_command(
    outcome: &SkillLoadOutcome,
    command: &str,
    workdir: &EnvironmentPathRef,
) -> Option<SkillMetadata> {
    let workdir = canonicalize_if_exists(workdir).await;
    let tokens = tokenize_command(command);

    if let Some(candidate) = detect_skill_script_run(outcome, tokens.as_slice(), &workdir).await {
        return Some(candidate);
    }

    detect_skill_doc_read(outcome, tokens.as_slice(), &workdir).await
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

async fn detect_skill_script_run(
    outcome: &SkillLoadOutcome,
    tokens: &[String],
    workdir: &EnvironmentPathRef,
) -> Option<SkillMetadata> {
    let script_token = script_run_token(tokens)?;
    let script_path = Path::new(script_token);
    let script_path = canonicalize_if_exists(&resolve_path_ref(workdir, script_path).await?).await;

    let mut path = Some(script_path);
    while let Some(candidate_path) = path {
        if let Some(candidate) = outcome.implicit_skills_by_scripts_dir.get(&candidate_path) {
            return Some(candidate.clone());
        }
        path = candidate_path.parent().await.ok().flatten();
    }

    None
}

async fn detect_skill_doc_read(
    outcome: &SkillLoadOutcome,
    tokens: &[String],
    workdir: &EnvironmentPathRef,
) -> Option<SkillMetadata> {
    if !command_reads_file(tokens) {
        return None;
    }

    for token in tokens.iter().skip(1) {
        if token.starts_with('-') {
            continue;
        }
        let path = Path::new(token);
        let candidate_path = canonicalize_if_exists(&resolve_path_ref(workdir, path).await?).await;
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

async fn canonicalize_if_exists(path: &EnvironmentPathRef) -> EnvironmentPathRef {
    path.canonicalize(/*sandbox*/ None)
        .await
        .unwrap_or_else(|_| path.clone())
}

async fn resolve_path_ref(workdir: &EnvironmentPathRef, path: &Path) -> Option<EnvironmentPathRef> {
    if path.is_absolute() {
        AbsolutePathBuf::from_absolute_path(path)
            .ok()
            .map(|path| workdir.with_path(path))
    } else {
        workdir.join(path).await.ok()
    }
}

#[cfg(test)]
#[path = "invocation_utils_tests.rs"]
mod tests;
