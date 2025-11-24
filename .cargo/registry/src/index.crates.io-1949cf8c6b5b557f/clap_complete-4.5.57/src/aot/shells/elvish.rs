use std::io::{Error, Write};

use clap::builder::StyledStr;
use clap::Command;

use crate::generator::{utils, Generator};
use crate::INTERNAL_ERROR_MSG;

/// Generate elvish completion file
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Elvish;

impl Generator for Elvish {
    fn file_name(&self, name: &str) -> String {
        format!("{name}.elv")
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn Write) {
        self.try_generate(cmd, buf)
            .expect("failed to write completion file");
    }

    fn try_generate(&self, cmd: &Command, buf: &mut dyn Write) -> Result<(), Error> {
        let bin_name = cmd
            .get_bin_name()
            .expect("crate::generate should have set the bin_name");

        let subcommands_cases = generate_inner(cmd, "");

        write!(
            buf,
            r#"
use builtin;
use str;

set edit:completion:arg-completer[{bin_name}] = {{|@words|
    fn spaces {{|n|
        builtin:repeat $n ' ' | str:join ''
    }}
    fn cand {{|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }}
    var command = '{bin_name}'
    for word $words[1..-1] {{
        if (str:has-prefix $word '-') {{
            break
        }}
        set command = $command';'$word
    }}
    var completions = [{subcommands_cases}
    ]
    $completions[$command]
}}
"#,
        )
    }
}

// Escape string inside single quotes
fn escape_string(string: &str) -> String {
    string.replace('\'', "''")
}

fn escape_help<T: ToString>(help: Option<&StyledStr>, data: T) -> String {
    match help {
        Some(help) => escape_string(&help.to_string().replace('\n', " ")),
        _ => data.to_string(),
    }
}

fn generate_inner(p: &Command, previous_command_name: &str) -> String {
    debug!("generate_inner");

    let command_names = if previous_command_name.is_empty() {
        vec![p.get_bin_name().expect(INTERNAL_ERROR_MSG).to_string()]
    } else {
        p.get_name_and_visible_aliases()
            .into_iter()
            .map(|name| format!("{previous_command_name};{name}"))
            .collect()
    };

    let mut completions = String::new();
    let preamble = String::from("\n            cand ");

    for option in p.get_opts() {
        if let Some(shorts) = option.get_short_and_visible_aliases() {
            let tooltip = escape_help(option.get_help(), shorts[0]);
            for short in shorts {
                completions.push_str(&preamble);
                completions.push_str(format!("-{short} '{tooltip}'").as_str());
            }
        }

        if let Some(longs) = option.get_long_and_visible_aliases() {
            let tooltip = escape_help(option.get_help(), longs[0]);
            for long in longs {
                completions.push_str(&preamble);
                completions.push_str(format!("--{long} '{tooltip}'").as_str());
            }
        }
    }

    for flag in utils::flags(p) {
        if let Some(shorts) = flag.get_short_and_visible_aliases() {
            let tooltip = escape_help(flag.get_help(), shorts[0]);
            for short in shorts {
                completions.push_str(&preamble);
                completions.push_str(format!("-{short} '{tooltip}'").as_str());
            }
        }

        if let Some(longs) = flag.get_long_and_visible_aliases() {
            let tooltip = escape_help(flag.get_help(), longs[0]);
            for long in longs {
                completions.push_str(&preamble);
                completions.push_str(format!("--{long} '{tooltip}'").as_str());
            }
        }
    }

    for subcommand in p.get_subcommands() {
        for name in subcommand.get_name_and_visible_aliases() {
            let tooltip = escape_help(subcommand.get_about(), name);

            completions.push_str(&preamble);
            completions.push_str(format!("{name} '{tooltip}'").as_str());
        }
    }

    let mut subcommands_cases = String::new();
    for command_name in &command_names {
        subcommands_cases.push_str(&format!(
            r"
        &'{}'= {{{}
        }}",
            &command_name, completions
        ));
    }

    for subcommand in p.get_subcommands() {
        for command_name in &command_names {
            let subcommand_subcommands_cases = generate_inner(subcommand, command_name);
            subcommands_cases.push_str(&subcommand_subcommands_cases);
        }
    }

    subcommands_cases
}
