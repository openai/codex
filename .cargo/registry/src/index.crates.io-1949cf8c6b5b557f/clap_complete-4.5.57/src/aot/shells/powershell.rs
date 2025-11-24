use std::io::{Error, Write};

use clap::builder::StyledStr;
use clap::{Arg, Command};

use crate::generator::{utils, Generator};
use crate::INTERNAL_ERROR_MSG;

/// Generate powershell completion file
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct PowerShell;

impl Generator for PowerShell {
    fn file_name(&self, name: &str) -> String {
        format!("_{name}.ps1")
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
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName '{bin_name}' -ScriptBlock {{
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        '{bin_name}'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {{
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {{
                break
        }}
        $element.Value
    }}) -join ';'

    $completions = @(switch ($command) {{{subcommands_cases}
    }})

    $completions.Where{{ $_.CompletionText -like "$wordToComplete*" }} |
        Sort-Object -Property ListItemText
}}
"#
        )
    }
}

// Escape string inside single quotes
fn escape_string(string: &str) -> String {
    string.replace('\'', "''").replace('’', "'’")
}

fn escape_help<T: ToString>(help: Option<&StyledStr>, data: T) -> String {
    if let Some(help) = help {
        let help_str = help.to_string();
        if !help_str.is_empty() {
            return escape_string(&help_str.replace('\n', " "));
        }
    }
    data.to_string()
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
    let preamble = String::from("\n            [CompletionResult]::new(");

    for option in p.get_opts() {
        generate_aliases(&mut completions, &preamble, option);
    }

    for flag in utils::flags(p) {
        generate_aliases(&mut completions, &preamble, &flag);
    }

    for subcommand in p.get_subcommands() {
        for name in subcommand.get_name_and_visible_aliases() {
            let tooltip = escape_help(subcommand.get_about(), name);
            completions.push_str(&preamble);
            completions.push_str(&format!(
                "'{name}', '{name}', [CompletionResultType]::ParameterValue, '{tooltip}')"
            ));
        }
    }

    let mut subcommands_cases = String::new();
    for command_name in &command_names {
        subcommands_cases.push_str(&format!(
            r"
        '{command_name}' {{{completions}
            break
        }}"
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

fn generate_aliases(completions: &mut String, preamble: &String, arg: &Arg) {
    use std::fmt::Write as _;

    if let Some(aliases) = arg.get_short_and_visible_aliases() {
        let tooltip = escape_help(arg.get_help(), aliases[0]);
        for alias in aliases {
            let _ = write!(
                completions,
                "{preamble}'-{alias}', '-{alias}{}', [CompletionResultType]::ParameterName, '{tooltip}')",
                // make PowerShell realize there is a difference between `-s` and `-S`
                if alias.is_uppercase() { " " } else { "" },
            );
        }
    }
    if let Some(aliases) = arg.get_long_and_visible_aliases() {
        let tooltip = escape_help(arg.get_help(), aliases[0]);
        for alias in aliases {
            let _ = write!(
                completions,
                "{preamble}'--{alias}', '--{alias}', [CompletionResultType]::ParameterName, '{tooltip}')"
            );
        }
    }
}
