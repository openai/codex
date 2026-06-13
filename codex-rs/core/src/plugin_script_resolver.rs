use std::path::Path;

use codex_analytics::PluginScriptSkill;
use codex_plugin::FirstPartyPluginRoot;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::shell::ShellType;
use crate::skills::SkillLoadOutcome;

#[derive(Debug)]
pub(crate) struct ResolvedPluginScript {
    pub(crate) plugin_id: String,
    pub(crate) script_path: String,
    pub(crate) skill: Option<PluginScriptSkill>,
}

pub(crate) fn resolve_plugin_script(
    plugin_roots: &[FirstPartyPluginRoot],
    skills_outcome: &SkillLoadOutcome,
    command: &str,
    cwd: &AbsolutePathBuf,
    shell_type: ShellType,
) -> Option<ResolvedPluginScript> {
    let script_token = script_token(command, shell_type)?;
    let script_path = Path::new(&script_token);
    let script_path = if script_path.is_absolute() {
        AbsolutePathBuf::try_from(script_path).ok()?
    } else {
        cwd.join(script_path)
    };
    let script_path = script_path.canonicalize().ok()?;
    script_path.as_path().is_file().then_some(())?;

    let (root, plugin_root) = plugin_roots
        .iter()
        .filter_map(|root| {
            let plugin_root = root.plugin_root.canonicalize().ok()?;
            script_path.strip_prefix(&plugin_root).ok()?;
            Some((root, plugin_root))
        })
        .max_by_key(|(_, plugin_root)| plugin_root.components().count())?;
    let relative = script_path.strip_prefix(plugin_root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    Some(ResolvedPluginScript {
        plugin_id: root.plugin_id.clone(),
        script_path: normalized_relative_path(relative)?,
        skill: skill_for_script(skills_outcome, &root.plugin_id, &script_path),
    })
}

fn skill_for_script(
    skills_outcome: &SkillLoadOutcome,
    plugin_id: &str,
    script_path: &Path,
) -> Option<PluginScriptSkill> {
    skills_outcome
        .skills
        .iter()
        .filter_map(|skill| {
            if skill.plugin_id.as_deref() != Some(plugin_id)
                || !skills_outcome.is_skill_enabled(skill)
            {
                return None;
            }
            let scripts_dir = skill.path_to_skills_md.parent()?.join("scripts");
            let scripts_dir = scripts_dir.canonicalize().ok()?;
            script_path.strip_prefix(&scripts_dir).ok()?;
            Some((skill, scripts_dir))
        })
        .max_by_key(|(_, scripts_dir)| scripts_dir.components().count())
        .map(|(skill, _)| PluginScriptSkill {
            skill_name: skill.name.clone(),
            skill_path: skill.path_to_skills_md.clone().into_path_buf(),
        })
}

fn script_token(command: &str, shell_type: ShellType) -> Option<String> {
    let tokens = command_tokens(command, shell_type)?;
    let program = tokens.first()?;
    let windows_shell = matches!(shell_type, ShellType::PowerShell | ShellType::Cmd);
    let basename = if windows_shell {
        program.rsplit(['/', '\\']).next()?.to_ascii_lowercase()
    } else {
        Path::new(program)
            .file_name()
            .and_then(|name| name.to_str())?
            .to_string()
    };
    let basename = if windows_shell {
        basename.strip_suffix(".exe").unwrap_or(&basename)
    } else {
        &basename
    };
    if !is_safe_script_candidate(program, shell_type) {
        return None;
    }
    let args = &tokens[1..];
    let path_qualified_program =
        Path::new(program).is_absolute() || program.contains('/') || program.contains('\\');
    let runner_script = if path_qualified_program {
        None
    } else {
        match basename {
            "python" | "python3" => script_after_allowed_options(args, &["-u"]),
            "bash" | "zsh" | "sh" => script_after_allowed_options(args, &["-e"]),
            "node" => args.first().filter(|arg| !arg.starts_with('-')).cloned(),
            "pwsh" | "powershell" => match args {
                [option, script, ..]
                    if matches!(option.to_ascii_lowercase().as_str(), "-file" | "-f") =>
                {
                    Some(script.clone())
                }
                _ => None,
            },
            _ => None,
        }
    };
    if let Some(runner_script) = runner_script {
        return is_safe_script_candidate(&runner_script, shell_type).then_some(runner_script);
    }
    if matches!(
        basename,
        "python" | "python3" | "bash" | "zsh" | "sh" | "node" | "pwsh" | "powershell"
    ) {
        return None;
    }

    path_qualified_program.then(|| program.clone())
}

fn is_safe_script_candidate(token: &str, shell_type: ShellType) -> bool {
    let expands_shell_paths =
        matches!(shell_type, ShellType::Bash | ShellType::Sh | ShellType::Zsh);
    !(expands_shell_paths && has_shell_path_expansion(token)
        || shell_type == ShellType::Zsh && token.starts_with('='))
}

fn has_shell_path_expansion(token: &str) -> bool {
    token.starts_with('~') || token.contains(['\\', '*', '?', '[', ']', '{', '}'])
}

fn script_after_allowed_options(args: &[String], allowed_options: &[&str]) -> Option<String> {
    let mut args = args.iter();
    loop {
        let arg = args.next()?;
        if !arg.starts_with('-') {
            return Some(arg.clone());
        }
        if !allowed_options.contains(&arg.as_str()) {
            return None;
        }
    }
}

fn command_tokens(command: &str, shell_type: ShellType) -> Option<Vec<String>> {
    match shell_type {
        ShellType::Bash | ShellType::Sh | ShellType::Zsh => {
            let tree = codex_shell_command::bash::try_parse_shell(command)?;
            let mut commands =
                codex_shell_command::bash::try_parse_word_only_commands_sequence(&tree, command)?;
            let [tokens] = commands.as_mut_slice() else {
                return None;
            };
            Some(std::mem::take(tokens))
        }
        ShellType::PowerShell => split_powershell_command(command),
        ShellType::Cmd => split_cmd_command(command),
    }
}

/// Splits one plain PowerShell-style command without treating backslashes as
/// escapes. Compound commands are rejected because lifecycle events attach to
/// the spawned shell process and cannot represent multiple child scripts.
fn split_powershell_command(command: &str) -> Option<Vec<String>> {
    if command.contains(['$', '~', '`', '(', ')', '{', '}', ',', '<', '>', '@', '#'])
        || matches!(command.trim_start().chars().next(), Some('\'' | '"'))
    {
        return None;
    }
    split_windows_command(command)
}

fn split_windows_command(command: &str) -> Option<Vec<String>> {
    let mut chars = command.chars().peekable();
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut saw_token = false;

    while let Some(ch) = chars.next() {
        if let Some(active_quote) = quote {
            if ch == '`' && active_quote == '"' {
                token.push(chars.next()?);
            } else if ch == active_quote {
                if chars.peek() == Some(&active_quote) {
                    token.push(active_quote);
                    chars.next();
                } else {
                    quote = None;
                }
            } else {
                token.push(ch);
            }
            continue;
        }

        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                saw_token = true;
            }
            '`' => {
                token.push(chars.next()?);
                saw_token = true;
            }
            ' ' | '\t' => {
                if saw_token {
                    tokens.push(std::mem::take(&mut token));
                    saw_token = false;
                }
            }
            '&' if tokens.is_empty() && !saw_token => {
                if !chars.peek().is_some_and(|next| next.is_whitespace()) {
                    return None;
                }
            }
            '&' | '|' | ';' | '\r' | '\n' => return None,
            _ => {
                token.push(ch);
                saw_token = true;
            }
        }
    }

    if quote.is_some() {
        return None;
    }
    if saw_token {
        tokens.push(token);
    }
    (!tokens.is_empty()).then_some(tokens)
}

fn split_cmd_command(command: &str) -> Option<Vec<String>> {
    if command.chars().any(|ch| {
        matches!(
            ch,
            '\'' | '`' | '^' | '%' | '!' | '&' | '|' | '<' | '>' | '(' | ')' | '\r' | '\n'
        )
    }) {
        return None;
    }
    split_windows_command(command)
}

fn normalized_relative_path(path: &Path) -> Option<String> {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => Some(value.to_str()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"))
}

#[cfg(test)]
#[path = "plugin_script_resolver_tests.rs"]
mod tests;
