use std::io::{Error, Write};

use clap::{builder, Arg, Command, ValueHint};

use crate::generator::{utils, Generator};

/// Generate fish completion file
///
/// Note: The fish generator currently only supports named options (-o/--option), not positional arguments.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fish;

impl Generator for Fish {
    fn file_name(&self, name: &str) -> String {
        format!("{name}.fish")
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn Write) {
        self.try_generate(cmd, buf)
            .expect("failed to write completion file");
    }

    fn try_generate(&self, cmd: &Command, buf: &mut dyn Write) -> Result<(), Error> {
        let bin_name = cmd
            .get_bin_name()
            .expect("crate::generate should have set the bin_name");

        let name = escape_name(bin_name);
        let mut needs_fn_name = &format!("__fish_{name}_needs_command")[..];
        let mut using_fn_name = &format!("__fish_{name}_using_subcommand")[..];
        // Given `git --git-dir somedir status`, using `__fish_seen_subcommand_from` won't help us
        // find out `status` is the real subcommand, and not `somedir`. However, when there are no subcommands,
        // there is no need to use our custom stubs.
        if cmd.has_subcommands() {
            gen_subcommand_helpers(&name, cmd, buf, needs_fn_name, using_fn_name);
        } else {
            needs_fn_name = "__fish_use_subcommand";
            using_fn_name = "__fish_seen_subcommand_from";
        }

        let mut buffer = String::new();
        gen_fish_inner(
            bin_name,
            &[],
            cmd,
            &mut buffer,
            needs_fn_name,
            using_fn_name,
        );
        write!(buf, "{buffer}")
    }
}

// Escape string inside single quotes
fn escape_string(string: &str, escape_comma: bool) -> String {
    let string = string.replace('\\', "\\\\").replace('\'', "\\'");
    if escape_comma {
        string.replace(',', "\\,")
    } else {
        string
    }
}

fn escape_help(help: &builder::StyledStr) -> String {
    escape_string(&help.to_string().replace('\n', " "), false)
}

fn escape_name(name: &str) -> String {
    name.replace('-', "_")
}

fn gen_fish_inner(
    root_command: &str,
    parent_commands: &[&str],
    cmd: &Command,
    buffer: &mut String,
    needs_fn_name: &str,
    using_fn_name: &str,
) {
    debug!("gen_fish_inner");
    // example :
    //
    // complete
    //      -c {command}
    //      -d "{description}"
    //      -s {short}
    //      -l {long}
    //      -a "{possible_arguments}"
    //      -r # if require parameter
    //      -f # don't use file completion
    //      -n "{needs_fn_name}"            # complete for command "myprog"
    //      -n "{using_fn_name} subcmd1"    # complete for command "myprog subcmd1"

    let mut basic_template = format!("complete -c {root_command}");

    if parent_commands.is_empty() {
        if cmd.has_subcommands() {
            basic_template.push_str(&format!(" -n \"{needs_fn_name}\""));
        }
    } else {
        let mut out = String::from(using_fn_name);
        match parent_commands {
            [] => unreachable!(),
            [command] => {
                out.push_str(&format!(" {command}"));
                if cmd.has_subcommands() {
                    out.push_str("; and not __fish_seen_subcommand_from");
                }
                let subcommands = cmd
                    .get_subcommands()
                    .flat_map(Command::get_name_and_visible_aliases);
                for name in subcommands {
                    out.push_str(&format!(" {name}"));
                }
            }
            [command, subcommand] => out.push_str(&format!(
                " {command}; and __fish_seen_subcommand_from {subcommand}"
            )),
            // HACK: Assuming subcommands are only nested less than 3 levels as more than that is
            // unwieldy and takes more effort to support.
            // For example, `rustup toolchain help install` is the longest valid command line of `rustup`
            // that uses nested subcommands, and it cannot receive any flags to it.
            _ => return,
        }
        basic_template.push_str(format!(" -n \"{out}\"").as_str());
    }

    debug!("gen_fish_inner: parent_commands={parent_commands:?}");

    for option in cmd.get_opts() {
        let mut template = basic_template.clone();

        if let Some(shorts) = option.get_short_and_visible_aliases() {
            for short in shorts {
                template.push_str(format!(" -s {short}").as_str());
            }
        }

        if let Some(longs) = option.get_long_and_visible_aliases() {
            for long in longs {
                template.push_str(format!(" -l {}", escape_string(long, false)).as_str());
            }
        }

        if let Some(data) = option.get_help() {
            template.push_str(&format!(" -d '{}'", escape_help(data)));
        }

        template.push_str(value_completion(option).as_str());

        buffer.push_str(template.as_str());
        buffer.push('\n');
    }

    for flag in utils::flags(cmd) {
        let mut template = basic_template.clone();

        if let Some(shorts) = flag.get_short_and_visible_aliases() {
            for short in shorts {
                template.push_str(format!(" -s {short}").as_str());
            }
        }

        if let Some(longs) = flag.get_long_and_visible_aliases() {
            for long in longs {
                template.push_str(format!(" -l {}", escape_string(long, false)).as_str());
            }
        }

        if let Some(data) = flag.get_help() {
            template.push_str(&format!(" -d '{}'", escape_help(data)));
        }

        buffer.push_str(template.as_str());
        buffer.push('\n');
    }

    let has_positionals = cmd.get_positionals().next().is_some();
    if !has_positionals {
        basic_template.push_str(" -f");
    }
    for subcommand in cmd.get_subcommands() {
        for subcommand_name in subcommand.get_name_and_visible_aliases() {
            let mut template = basic_template.clone();

            template.push_str(format!(" -a \"{subcommand_name}\"").as_str());

            if let Some(data) = subcommand.get_about() {
                template.push_str(format!(" -d '{}'", escape_help(data)).as_str());
            }

            buffer.push_str(template.as_str());
            buffer.push('\n');
        }
    }

    // generate options of subcommands
    for subcommand in cmd.get_subcommands() {
        for subcommand_name in subcommand.get_name_and_visible_aliases() {
            let mut parent_commands: Vec<_> = parent_commands.into();
            parent_commands.push(subcommand_name);
            gen_fish_inner(
                root_command,
                &parent_commands,
                subcommand,
                buffer,
                needs_fn_name,
                using_fn_name,
            );
        }
    }
}

/// Print fish's helpers for easy handling subcommands.
fn gen_subcommand_helpers(
    bin_name: &str,
    cmd: &Command,
    buf: &mut dyn Write,
    needs_fn_name: &str,
    using_fn_name: &str,
) {
    let mut optspecs = String::new();
    let cmd_opts = cmd.get_arguments().filter(|a| !a.is_positional());
    for option in cmd_opts {
        optspecs.push(' ');
        let mut has_short = false;
        if let Some(short) = option.get_short() {
            has_short = true;
            optspecs.push(short);
        }

        if let Some(long) = option.get_long() {
            if has_short {
                optspecs.push('/');
            }
            optspecs.push_str(&escape_string(long, false));
        }

        let is_an_option = option
            .get_num_args()
            .map(|r| r.takes_values())
            .unwrap_or(true);
        if is_an_option {
            optspecs.push('=');
        }
    }
    let optspecs_fn_name = format!("__fish_{bin_name}_global_optspecs");
    write!(
        buf,
        "\
        # Print an optspec for argparse to handle cmd's options that are independent of any subcommand.\n\
        function {optspecs_fn_name}\n\
        \tstring join \\n{optspecs}\n\
        end\n\n\
        function {needs_fn_name}\n\
        \t# Figure out if the current invocation already has a command.\n\
        \tset -l cmd (commandline -opc)\n\
        \tset -e cmd[1]\n\
        \targparse -s ({optspecs_fn_name}) -- $cmd 2>/dev/null\n\
        \tor return\n\
        \tif set -q argv[1]\n\
        \t\t# Also print the command, so this can be used to figure out what it is.\n\
        \t\techo $argv[1]\n\
        \t\treturn 1\n\
        \tend\n\
        \treturn 0\n\
        end\n\n\
        function {using_fn_name}\n\
        \tset -l cmd ({needs_fn_name})\n\
        \ttest -z \"$cmd\"\n\
        \tand return 1\n\
        \tcontains -- $cmd[1] $argv\n\
        end\n\n\
    ").expect("failed to write completion file");
}

fn value_completion(option: &Arg) -> String {
    if !option.get_num_args().expect("built").takes_values() {
        return "".to_string();
    }

    if let Some(data) = utils::possible_values(option) {
        // We return the possible values with their own empty description e.g. "a\t''\nb\t''"
        // this makes sure that a and b don't get the description of the option or argument
        format!(
            " -r -f -a \"{}\"",
            data.iter()
                .filter_map(|value| if value.is_hide_set() {
                    None
                } else {
                    // The help text after \t is wrapped in '' to make sure that the it is taken literally
                    // and there is no command substitution or variable expansion resulting in unexpected errors
                    Some(format!(
                        "{}\\t'{}'",
                        escape_string(value.get_name(), true).as_str(),
                        escape_help(value.get_help().unwrap_or_default())
                    ))
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        // NB! If you change this, please also update the table in `ValueHint` documentation.
        match option.get_value_hint() {
            ValueHint::Unknown => " -r",
            // fish has no built-in support to distinguish these
            ValueHint::AnyPath | ValueHint::FilePath | ValueHint::ExecutablePath => " -r -F",
            ValueHint::DirPath => " -r -f -a \"(__fish_complete_directories)\"",
            // It seems fish has no built-in support for completing command + arguments as
            // single string (CommandString). Complete just the command name.
            ValueHint::CommandString | ValueHint::CommandName => {
                " -r -f -a \"(__fish_complete_command)\""
            }
            ValueHint::Username => " -r -f -a \"(__fish_complete_users)\"",
            ValueHint::Hostname => " -r -f -a \"(__fish_print_hostnames)\"",
            // Disable completion for others
            _ => " -r -f",
        }
        .to_string()
    }
}
