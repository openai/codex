use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use codex_protocol::models::ResponseItem;
use serde::Deserialize;

use crate::analytics_client::InvocationType;
use crate::analytics_client::SkillInvocation;
use crate::codex::TurnContext;
use crate::skills::SkillMetadata;

#[derive(Clone)]
pub(crate) struct ImplicitSkillCandidate {
    pub(crate) invocation: SkillInvocation,
}

#[derive(Default)]
pub(crate) struct ImplicitSkillDetector {
    pub(crate) by_scripts_dir: HashMap<PathBuf, ImplicitSkillCandidate>,
    pub(crate) by_skill_doc_path: HashMap<PathBuf, ImplicitSkillCandidate>,
}

pub(crate) struct ImplicitInvocationContext {
    pub(crate) detector: ImplicitSkillDetector,
}

#[derive(Deserialize)]
struct ShellCommandDetectionArgs {
    command: String,
    workdir: Option<String>,
}

#[derive(Deserialize)]
struct ExecCommandDetectionArgs {
    cmd: String,
    workdir: Option<String>,
}

pub(crate) fn build_implicit_invocation_context(
    skills: Vec<SkillMetadata>,
) -> Option<ImplicitInvocationContext> {
    if skills.is_empty() {
        return None;
    }

    let mut detector = ImplicitSkillDetector::default();
    for skill in skills {
        let invocation = SkillInvocation {
            skill_name: skill.name,
            skill_scope: skill.scope,
            skill_path: skill.path,
            invocation_type: InvocationType::Implicit,
        };
        let candidate = ImplicitSkillCandidate { invocation };

        let skill_doc_path = normalize_path(candidate.invocation.skill_path.as_path());
        detector
            .by_skill_doc_path
            .insert(skill_doc_path, candidate.clone());

        if let Some(skill_dir) = candidate.invocation.skill_path.parent() {
            let scripts_dir = normalize_path(&skill_dir.join("scripts"));
            detector.by_scripts_dir.insert(scripts_dir, candidate);
        }
    }

    Some(ImplicitInvocationContext { detector })
}

pub(crate) fn detect_implicit_skill_invocation(
    detector: &ImplicitSkillDetector,
    turn_context: &TurnContext,
    item: &ResponseItem,
) -> Option<ImplicitSkillCandidate> {
    let ResponseItem::FunctionCall {
        name, arguments, ..
    } = item
    else {
        return None;
    };
    let (command, workdir) = parse_implicit_detection_command(name, arguments)?;
    let workdir = turn_context.resolve_path(workdir);
    let workdir = normalize_path(workdir.as_path());
    let tokens = tokenize_command(command.as_str());

    if let Some(candidate) = detect_skill_script_run(detector, tokens.as_slice(), workdir.as_path())
    {
        return Some(candidate);
    }

    if let Some(candidate) = detect_skill_doc_read(detector, tokens.as_slice(), workdir.as_path()) {
        return Some(candidate);
    }

    None
}

fn parse_implicit_detection_command(
    tool_name: &str,
    arguments: &str,
) -> Option<(String, Option<String>)> {
    match tool_name {
        "shell_command" => serde_json::from_str::<ShellCommandDetectionArgs>(arguments)
            .ok()
            .map(|args| (args.command, args.workdir)),
        "exec_command" => serde_json::from_str::<ExecCommandDetectionArgs>(arguments)
            .ok()
            .map(|args| (args.cmd, args.workdir)),
        _ => None,
    }
}

fn tokenize_command(command: &str) -> Vec<String> {
    shlex::split(command).unwrap_or_else(|| {
        command
            .split_whitespace()
            .map(std::string::ToString::to_string)
            .collect()
    })
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

    let mut script_token: Option<&str> = None;
    for token in tokens.iter().skip(1) {
        if token == "--" {
            continue;
        }
        if token.starts_with('-') {
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
    detector: &ImplicitSkillDetector,
    tokens: &[String],
    workdir: &Path,
) -> Option<ImplicitSkillCandidate> {
    let script_token = script_run_token(tokens)?;
    let script_path = Path::new(script_token);
    let script_path = if script_path.is_absolute() {
        script_path.to_path_buf()
    } else {
        workdir.join(script_path)
    };
    let script_path = normalize_path(script_path.as_path());

    for ancestor in script_path.ancestors() {
        if let Some(candidate) = detector.by_scripts_dir.get(ancestor) {
            return Some(candidate.clone());
        }
    }

    None
}

fn detect_skill_doc_read(
    detector: &ImplicitSkillDetector,
    tokens: &[String],
    workdir: &Path,
) -> Option<ImplicitSkillCandidate> {
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
        if let Some(candidate) = detector.by_skill_doc_path.get(&candidate_path) {
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
mod tests {
    use super::ImplicitSkillCandidate;
    use super::ImplicitSkillDetector;
    use super::InvocationType;
    use super::SkillInvocation;
    use super::detect_skill_doc_read;
    use super::detect_skill_script_run;
    use super::normalize_path;
    use super::script_run_token;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn script_run_detection_matches_runner_plus_extension() {
        let tokens = vec![
            "python3".to_string(),
            "-u".to_string(),
            "scripts/fetch_comments.py".to_string(),
        ];

        assert_eq!(script_run_token(&tokens).is_some(), true);
    }

    #[test]
    fn script_run_detection_excludes_python_c() {
        let tokens = vec![
            "python3".to_string(),
            "-c".to_string(),
            "print(1)".to_string(),
        ];

        assert_eq!(script_run_token(&tokens).is_some(), false);
    }

    #[test]
    fn skill_doc_read_detection_matches_absolute_path() {
        let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
        let normalized_skill_doc_path = normalize_path(skill_doc_path.as_path());
        let invocation = SkillInvocation {
            skill_name: "test-skill".to_string(),
            skill_scope: codex_protocol::protocol::SkillScope::User,
            skill_path: skill_doc_path,
            invocation_type: InvocationType::Implicit,
        };
        let candidate = ImplicitSkillCandidate { invocation };

        let detector = ImplicitSkillDetector {
            by_scripts_dir: HashMap::new(),
            by_skill_doc_path: HashMap::from([(normalized_skill_doc_path, candidate)]),
        };

        let tokens = vec![
            "cat".to_string(),
            "/tmp/skill-test/SKILL.md".to_string(),
            "|".to_string(),
            "head".to_string(),
        ];
        let found = detect_skill_doc_read(&detector, &tokens, Path::new("/tmp"));

        assert_eq!(
            found.map(|value| value.invocation.skill_name),
            Some("test-skill".to_string())
        );
    }

    #[test]
    fn skill_script_run_detection_matches_relative_path_from_skill_root() {
        let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
        let scripts_dir = normalize_path(Path::new("/tmp/skill-test/scripts"));
        let invocation = SkillInvocation {
            skill_name: "test-skill".to_string(),
            skill_scope: codex_protocol::protocol::SkillScope::User,
            skill_path: skill_doc_path,
            invocation_type: InvocationType::Implicit,
        };
        let candidate = ImplicitSkillCandidate { invocation };

        let detector = ImplicitSkillDetector {
            by_scripts_dir: HashMap::from([(scripts_dir, candidate)]),
            by_skill_doc_path: HashMap::new(),
        };
        let tokens = vec![
            "python3".to_string(),
            "scripts/fetch_comments.py".to_string(),
        ];

        let found = detect_skill_script_run(&detector, &tokens, Path::new("/tmp/skill-test"));

        assert_eq!(
            found.map(|value| value.invocation.skill_name),
            Some("test-skill".to_string())
        );
    }

    #[test]
    fn skill_script_run_detection_matches_absolute_path_from_any_workdir() {
        let skill_doc_path = PathBuf::from("/tmp/skill-test/SKILL.md");
        let scripts_dir = normalize_path(Path::new("/tmp/skill-test/scripts"));
        let invocation = SkillInvocation {
            skill_name: "test-skill".to_string(),
            skill_scope: codex_protocol::protocol::SkillScope::User,
            skill_path: skill_doc_path,
            invocation_type: InvocationType::Implicit,
        };
        let candidate = ImplicitSkillCandidate { invocation };

        let detector = ImplicitSkillDetector {
            by_scripts_dir: HashMap::from([(scripts_dir, candidate)]),
            by_skill_doc_path: HashMap::new(),
        };
        let tokens = vec![
            "python3".to_string(),
            "/tmp/skill-test/scripts/fetch_comments.py".to_string(),
        ];

        let found = detect_skill_script_run(&detector, &tokens, Path::new("/tmp/other"));

        assert_eq!(
            found.map(|value| value.invocation.skill_name),
            Some("test-skill".to_string())
        );
    }
}
