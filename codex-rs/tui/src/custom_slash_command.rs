use std::fs;
use std::path::Path;
use std::path::PathBuf;

use tracing::warn;

const COMMANDS_DIR: &str = "commands";
const MARKDOWN_EXTENSION: &str = "md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CustomSlashCommand {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) argument_hint: Option<String>,
    pub(crate) path: PathBuf,
    prompt: String,
}

impl CustomSlashCommand {
    pub(crate) fn expanded_prompt(&self, args: &str) -> String {
        expand_placeholders(&self.prompt, args)
    }
}

#[derive(Default)]
struct CommandMetadata {
    description: Option<String>,
    argument_hint: Option<String>,
}

pub(crate) fn load_custom_slash_commands(codex_home: &Path) -> Vec<CustomSlashCommand> {
    let root = codex_home.join(COMMANDS_DIR);
    let mut commands = Vec::new();
    if let Err(err) = collect_commands(&root, &root, &mut commands)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        warn!(
            "failed to load custom slash commands from {}: {err}",
            root.display()
        );
    }
    commands.sort_by(|left, right| left.name.cmp(&right.name).then(left.path.cmp(&right.path)));
    commands.dedup_by(|left, right| left.name == right.name);
    commands
}

fn collect_commands(
    root: &Path,
    current: &Path,
    commands: &mut Vec<CustomSlashCommand>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_commands(root, &path, commands)?;
        } else if file_type.is_file()
            && path.extension().and_then(|ext| ext.to_str()) == Some(MARKDOWN_EXTENSION)
            && let Some(command) = command_from_path(root, &path)
        {
            commands.push(command);
        }
    }
    Ok(())
}

fn command_from_path(root: &Path, path: &Path) -> Option<CustomSlashCommand> {
    path.strip_prefix(root).ok()?;
    let stem = path.file_stem()?.to_string_lossy();
    if !is_valid_command_name(&stem) {
        return None;
    }
    let content = fs::read_to_string(path).ok()?;
    let (metadata, prompt) = parse_command_file(&content);
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return None;
    }
    let description = metadata
        .description
        .unwrap_or_else(|| default_description(&prompt));
    Some(CustomSlashCommand {
        name: stem.to_string(),
        description,
        argument_hint: metadata.argument_hint,
        path: path.to_path_buf(),
        prompt,
    })
}

fn is_valid_command_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn parse_command_file(content: &str) -> (CommandMetadata, String) {
    let normalized = content.replace("\r\n", "\n");
    let Some(rest) = normalized.strip_prefix("---\n") else {
        return (CommandMetadata::default(), normalized);
    };
    let Some(end) = rest.find("\n---") else {
        return (CommandMetadata::default(), normalized);
    };
    let frontmatter = &rest[..end];
    let body_start = end + "\n---".len();
    let body = rest[body_start..]
        .strip_prefix('\n')
        .unwrap_or(&rest[body_start..]);
    (parse_frontmatter(frontmatter), body.to_string())
}

fn parse_frontmatter(frontmatter: &str) -> CommandMetadata {
    let mut metadata = CommandMetadata::default();
    for line in frontmatter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "description" => metadata.description = Some(value.to_string()),
            "argument-hint" => metadata.argument_hint = Some(value.to_string()),
            _ => {}
        }
    }
    metadata
}

fn default_description(prompt: &str) -> String {
    prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches('#').trim())
        .filter(|line| !line.is_empty())
        .unwrap_or("custom prompt")
        .to_string()
}

fn expand_placeholders(prompt: &str, args: &str) -> String {
    let positional_args = shlex::split(args)
        .unwrap_or_else(|| args.split_whitespace().map(ToString::to_string).collect());
    let mut expanded = String::with_capacity(prompt.len() + args.len());
    let mut chars = prompt.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        if ch != '$' {
            expanded.push(ch);
            continue;
        }
        let rest = &prompt[idx..];
        if rest.starts_with("$ARGUMENTS") {
            expanded.push_str(args);
            for _ in 0.."ARGUMENTS".len() {
                chars.next();
            }
            continue;
        }
        let mut arg_number = String::new();
        while let Some((_, next)) = chars.peek().copied() {
            if !next.is_ascii_digit() {
                break;
            }
            arg_number.push(next);
            chars.next();
        }
        if arg_number.is_empty() {
            expanded.push('$');
            continue;
        }
        if let Ok(index) = arg_number.parse::<usize>()
            && index > 0
            && let Some(value) = positional_args.get(index - 1)
        {
            expanded.push_str(value);
        }
    }
    expanded
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn loads_private_commands_from_codex_home() {
        let codex_home = TempDir::new().expect("tempdir");
        let commands_dir = codex_home.path().join("commands").join("db");
        fs::create_dir_all(&commands_dir).expect("commands dir");
        fs::write(
            commands_dir.join("migrate.md"),
            "---\ndescription: Run a migration review\nargument-hint: <revision>\n---\nReview migration $ARGUMENTS.",
        )
        .expect("command file");

        let commands = load_custom_slash_commands(codex_home.path());

        assert_eq!(
            commands,
            vec![CustomSlashCommand {
                name: "migrate".to_string(),
                description: "Run a migration review".to_string(),
                argument_hint: Some("<revision>".to_string()),
                path: commands_dir.join("migrate.md"),
                prompt: "Review migration $ARGUMENTS.".to_string(),
            }]
        );
    }

    #[test]
    fn expands_all_and_positional_arguments() {
        let command = CustomSlashCommand {
            name: "review-pr".to_string(),
            description: "Review PR".to_string(),
            argument_hint: None,
            path: PathBuf::from("review-pr.md"),
            prompt: "Review PR $1 with priority $2. Raw: $ARGUMENTS.".to_string(),
        };

        assert_eq!(
            command.expanded_prompt("456 high"),
            "Review PR 456 with priority high. Raw: 456 high."
        );
    }
}
