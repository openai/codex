//! Locate a simple named-setting token without changing shell-quoted arguments around it.

use std::ops::Range;

use crate::slash_command::SlashCommand;
use crate::slash_command_settings;

const PROBE: &str = "__codex_completion_probe__";

pub(super) fn target(
    line: &str,
    cursor: usize,
    args_start: usize,
    command: SlashCommand,
) -> Option<(Range<usize>, Vec<String>)> {
    let mut settings = slash_command_settings::for_command(command);
    if settings.values.is_empty() && settings.switches.is_empty() && settings.booleans.is_empty() {
        return None;
    }
    let start = line[args_start..cursor]
        .rfind([' ', '\t', '\n'])
        .map_or(args_start, |index| args_start + index + 1);
    let end = cursor
        + line[cursor..]
            .find([' ', '\t', '\n'])
            .unwrap_or(line.len() - cursor);
    let token = &line[start..end];
    let editable_end = token
        .find('=')
        .filter(|&i| cursor <= start + i)
        .unwrap_or(token.len());
    if token.starts_with('#') || token[..editable_end].contains(['\'', '"', '\\']) {
        return None;
    }
    // The probe rejects positions inside quotes, escaped whitespace, and comments.
    let prefix = &line[args_start..start];
    let mut words = shlex::split(&format!("{prefix}{PROBE}"))?;
    if words.pop().as_deref() != Some(PROBE) || words.len() != shlex::split(prefix)?.len() {
        return None;
    }
    if command == SlashCommand::Mcp {
        let (subcommand, _) = prefix.trim_start().split_once(' ')?;
        if !subcommand.eq_ignore_ascii_case("login") {
            return None;
        }
        words.remove(0);
    }

    let mut used = Vec::new();
    let mut pending_value = None;
    let mut positionals = 0;
    for word in &words {
        if pending_value.take().is_some() && !word.starts_with("--") {
            continue;
        }
        if word == "--" {
            return None;
        }
        if word.starts_with("--") {
            let (name, inline) = word.split_once('=').unwrap_or((word.as_str(), ""));
            used.push(name);
            if settings.values.contains(&name) && inline.is_empty() {
                pending_value = Some(name);
            }
        } else {
            positionals += 1;
        }
    }
    let tail = shlex::split(&line[start..])?;
    let tail = tail.get(usize::from(!token.is_empty())..)?;
    if token == "--" && !tail.is_empty() {
        return None;
    }
    let terminator = tail
        .iter()
        .position(|word| word == "--")
        .unwrap_or(tail.len());
    positionals += tail.len().saturating_sub(terminator + 1);
    for word in &tail[..terminator] {
        if word.starts_with("--") {
            used.push(word.split_once('=').map_or(word.as_str(), |(name, _)| name));
        } else if matches!(command, SlashCommand::Title | SlashCommand::Statusline) {
            positionals += 1;
        }
    }
    if positionals > 0 && matches!(command, SlashCommand::Title | SlashCommand::Statusline) {
        used.push("--clear");
    }

    let value_target = |name: &str, range: Range<usize>| {
        let own_use = usize::from(pending_value == Some(name) && !token.contains('='));
        let occupied = range.is_empty() && tail.first().is_some_and(|word| !word.starts_with("--"));
        if occupied || used.iter().filter(|used| **used == name).count() > own_use {
            return None;
        }
        let values = slash_command_settings::value_choices(command, name);
        (!values.is_empty()).then(|| (range, values.iter().copied().map(str::to_owned).collect()))
    };
    if let Some(name) = pending_value
        && !token.starts_with("--")
    {
        return value_target(name, start..end);
    }
    if !token.starts_with('-') {
        return None;
    }
    if let Some(equal) = token.find('=') {
        let name = &token[..equal];
        if cursor > start + equal {
            return value_target(name, start + equal + 1..end);
        }
        settings.switches = &[];
        settings.booleans = &[];
        return option_names(settings, &used).map(|names| (start..start + equal, names));
    }
    option_names(settings, &used).map(|names| (start..end, names))
}

fn option_names(settings: slash_command_settings::Settings, used: &[&str]) -> Option<Vec<String>> {
    let mut names: Vec<String> = settings
        .values
        .iter()
        .chain(settings.switches)
        .map(|name| (*name).to_owned())
        .collect();
    for boolean in settings.booleans {
        let positive = format!("--{boolean}");
        let negative = format!("--no-{boolean}");
        if !used.contains(&positive.as_str()) && !used.contains(&negative.as_str()) {
            names.extend([positive, negative]);
        }
    }
    names.retain(|name| !used.contains(&name.as_str()));
    (!names.is_empty()).then_some(names)
}
