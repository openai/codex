//! Shared tokenization for slash commands with positional arguments and named settings.
//!
//! Values may follow their setting or use `=`, switches never consume a value, and boolean
//! settings accept both `--name` and `--no-name`. A bare `--` leaves all remaining tokens
//! positional.

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ParsedCommandArgs {
    pub(super) positionals: Vec<String>,
    named: Vec<NamedArgument>,
}

#[derive(Debug, PartialEq, Eq)]
struct NamedArgument {
    name: String,
    value: NamedArgumentValue,
}

#[derive(Debug, PartialEq, Eq)]
enum NamedArgumentValue {
    Value(String),
    Switch,
    Boolean { value: bool },
}

impl ParsedCommandArgs {
    pub(super) fn value(&self, name: &str) -> Option<&str> {
        self.named
            .iter()
            .find(|argument| argument.name == name)
            .and_then(|argument| match &argument.value {
                NamedArgumentValue::Value(value) => Some(value.as_str()),
                NamedArgumentValue::Switch | NamedArgumentValue::Boolean { .. } => None,
            })
    }
    pub(super) fn has_switch(&self, name: &str) -> bool {
        self.named
            .iter()
            .any(|argument| argument.name == name && argument.value == NamedArgumentValue::Switch)
    }

    pub(super) fn boolean(&self, name: &str) -> Option<bool> {
        let name = format!("--{name}");
        self.named
            .iter()
            .find(|argument| argument.name == name)
            .and_then(|argument| match &argument.value {
                NamedArgumentValue::Boolean { value } => Some(*value),
                NamedArgumentValue::Value(_) | NamedArgumentValue::Switch => None,
            })
    }
}

pub(super) fn parse(
    args: &str,
    value_settings: &[&str],
    switches: &[&str],
    boolean_settings: &[&str],
) -> Result<ParsedCommandArgs, String> {
    let mut words = shlex::split(args)
        .ok_or_else(|| "Invalid command arguments. Check the quoting.".to_string())?
        .into_iter()
        .peekable();
    let mut parsed = ParsedCommandArgs::default();
    let mut parse_settings = true;
    while let Some(word) = words.next() {
        if parse_settings && word == "--" {
            parse_settings = false;
        } else if parse_settings && word.starts_with("--") {
            let argument = parse_named_argument(
                &word,
                &mut words,
                value_settings,
                switches,
                boolean_settings,
            )?;
            if let Some(existing) = parsed
                .named
                .iter()
                .find(|existing| existing.name == argument.name)
            {
                if let (
                    NamedArgumentValue::Boolean {
                        value: existing_value,
                    },
                    NamedArgumentValue::Boolean { value },
                ) = (&existing.value, &argument.value)
                    && existing_value != value
                {
                    let name = argument.name.trim_start_matches("--");
                    return Err(format!("Specify only one of --{name} or --no-{name}."));
                }
                return Err(format!(
                    "Command setting `{}` was provided more than once.",
                    argument.name
                ));
            }
            parsed.named.push(argument);
        } else {
            parsed.positionals.push(word);
        }
    }
    Ok(parsed)
}

pub(super) fn parse_settings(
    args: &str,
    command: crate::slash_command::SlashCommand,
) -> Result<ParsedCommandArgs, String> {
    let settings = crate::slash_command_settings::for_command(command);
    parse(args, settings.values, settings.switches, settings.booleans)
}

fn parse_named_argument(
    word: &str,
    words: &mut std::iter::Peekable<std::vec::IntoIter<String>>,
    value_settings: &[&str],
    switches: &[&str],
    boolean_settings: &[&str],
) -> Result<NamedArgument, String> {
    let (name, inline_value) = word
        .split_once('=')
        .map_or((word, None), |(name, value)| (name, Some(value)));
    if value_settings.contains(&name) {
        let value = inline_value
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .or_else(|| words.next_if(|value| !value.starts_with("--")))
            .ok_or_else(|| format!("Command setting `{name}` requires a value."))?;
        return Ok(NamedArgument {
            name: name.to_string(),
            value: NamedArgumentValue::Value(value),
        });
    }
    if switches.contains(&name) {
        if inline_value.is_some() {
            return Err(format!("Command setting `{name}` does not take a value."));
        }
        return Ok(NamedArgument {
            name: name.to_string(),
            value: NamedArgumentValue::Switch,
        });
    }
    for setting in boolean_settings {
        let positive = format!("--{setting}");
        let negative = format!("--no-{setting}");
        if name == positive || name == negative {
            if inline_value.is_some() {
                return Err(format!("Command setting `{name}` does not take a value."));
            }
            return Ok(NamedArgument {
                name: positive,
                value: NamedArgumentValue::Boolean {
                    value: name != negative,
                },
            });
        }
    }
    Err(format!("Unknown command setting `{name}`."))
}

#[cfg(test)]
#[path = "slash_command_args_tests.rs"]
mod tests;
