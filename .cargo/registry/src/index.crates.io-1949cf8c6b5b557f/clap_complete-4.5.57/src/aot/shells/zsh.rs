use std::io::{Error, Write};

use clap::{Arg, ArgAction, Command, ValueHint};

use crate::generator::{utils, Generator};
use crate::INTERNAL_ERROR_MSG;

/// Generate zsh completion file
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Zsh;

impl Generator for Zsh {
    fn file_name(&self, name: &str) -> String {
        format!("_{name}")
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn Write) {
        self.try_generate(cmd, buf)
            .expect("failed to write completion file");
    }

    fn try_generate(&self, cmd: &Command, buf: &mut dyn Write) -> Result<(), Error> {
        let bin_name = cmd
            .get_bin_name()
            .expect("crate::generate should have set the bin_name");

        write!(
            buf,
            "#compdef {name}

autoload -U is-at-least

_{name}() {{
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext=\"$curcontext\" state line
    {initial_args}{subcommands}
}}

{subcommand_details}

if [ \"$funcstack[1]\" = \"_{name}\" ]; then
    _{name} \"$@\"
else
    compdef _{name} {name}
fi
",
            name = bin_name,
            initial_args = get_args_of(cmd, None),
            subcommands = get_subcommands_of(cmd),
            subcommand_details = subcommand_details(cmd)
        )
    }
}

// Displays the commands of a subcommand
// (( $+functions[_[bin_name_underscore]_commands] )) ||
// _[bin_name_underscore]_commands() {
//     local commands; commands=(
//         '[arg_name]:[arg_help]'
//     )
//     _describe -t commands '[bin_name] commands' commands "$@"
//
// Where the following variables are present:
//    [bin_name_underscore]: The full space delineated bin_name, where spaces have been replaced by
//                           underscore characters
//    [arg_name]: The name of the subcommand
//    [arg_help]: The help message of the subcommand
//    [bin_name]: The full space delineated bin_name
//
// Here's a snippet from rustup:
//
// (( $+functions[_rustup_commands] )) ||
// _rustup_commands() {
//     local commands; commands=(
//      'show:Show the active and installed toolchains'
//      'update:Update Rust toolchains'
//      # ... snip for brevity
//      'help:Print this message or the help of the given subcommand(s)'
//     )
//     _describe -t commands 'rustup commands' commands "$@"
//
fn subcommand_details(p: &Command) -> String {
    debug!("subcommand_details");

    let bin_name = p
        .get_bin_name()
        .expect("crate::generate should have set the bin_name");

    let mut ret = vec![];

    // First we do ourself
    let parent_text = format!(
        "\
(( $+functions[_{bin_name_underscore}_commands] )) ||
_{bin_name_underscore}_commands() {{
    local commands; commands=({subcommands_and_args})
    _describe -t commands '{bin_name} commands' commands \"$@\"
}}",
        bin_name_underscore = bin_name.replace(' ', "__"),
        bin_name = bin_name,
        subcommands_and_args = subcommands_of(p)
    );
    ret.push(parent_text);

    // Next we start looping through all the children, grandchildren, etc.
    let mut all_subcommand_bins: Vec<_> = utils::all_subcommands(p)
        .into_iter()
        .map(|(_sc_name, bin_name)| bin_name)
        .collect();

    all_subcommand_bins.sort();
    all_subcommand_bins.dedup();

    for bin_name in &all_subcommand_bins {
        debug!("subcommand_details:iter: bin_name={bin_name}");

        ret.push(format!(
            "\
(( $+functions[_{bin_name_underscore}_commands] )) ||
_{bin_name_underscore}_commands() {{
    local commands; commands=({subcommands_and_args})
    _describe -t commands '{bin_name} commands' commands \"$@\"
}}",
            bin_name_underscore = bin_name.replace(' ', "__"),
            bin_name = bin_name,
            subcommands_and_args =
                subcommands_of(parser_of(p, bin_name).expect(INTERNAL_ERROR_MSG))
        ));
    }

    ret.join("\n")
}

// Generates subcommand completions in form of
//
//         '[arg_name]:[arg_help]'
//
// Where:
//    [arg_name]: the subcommand's name
//    [arg_help]: the help message of the subcommand
//
// A snippet from rustup:
//         'show:Show the active and installed toolchains'
//      'update:Update Rust toolchains'
fn subcommands_of(p: &Command) -> String {
    debug!("subcommands_of");

    let mut segments = vec![];

    fn add_subcommands(subcommand: &Command, name: &str, ret: &mut Vec<String>) {
        debug!("add_subcommands");

        let text = format!(
            "'{name}:{help}' \\",
            name = name,
            help = escape_help(&subcommand.get_about().unwrap_or_default().to_string())
        );

        ret.push(text);
    }

    // The subcommands
    for command in p.get_subcommands() {
        debug!("subcommands_of:iter: subcommand={}", command.get_name());

        add_subcommands(command, command.get_name(), &mut segments);

        for alias in command.get_visible_aliases() {
            add_subcommands(command, alias, &mut segments);
        }
    }

    // Surround the text with newlines for proper formatting.
    // We need this to prevent weirdly formatted `command=(\n        \n)` sections.
    // When there are no (sub-)commands.
    if !segments.is_empty() {
        segments.insert(0, "".to_string());
        segments.push("    ".to_string());
    }

    segments.join("\n")
}

// Get's the subcommand section of a completion file
// This looks roughly like:
//
// case $state in
// ([bin_name]_args)
//     curcontext=\"${curcontext%:*:*}:[name_hyphen]-command-$words[1]:\"
//     case $line[1] in
//
//         ([name])
//         _arguments -C -s -S \
//             [subcommand_args]
//         && ret=0
//
//         [RECURSIVE_CALLS]
//
//         ;;",
//
//         [repeat]
//
//     esac
// ;;
// esac",
//
// Where the following variables are present:
//    [name] = The subcommand name in the form of "install" for "rustup toolchain install"
//    [bin_name] = The full space delineated bin_name such as "rustup toolchain install"
//    [name_hyphen] = The full space delineated bin_name, but replace spaces with hyphens
//    [repeat] = From the same recursive calls, but for all subcommands
//    [subcommand_args] = The same as zsh::get_args_of
fn get_subcommands_of(parent: &Command) -> String {
    debug!(
        "get_subcommands_of: Has subcommands...{:?}",
        parent.has_subcommands()
    );

    if !parent.has_subcommands() {
        return String::new();
    }

    let subcommand_names = utils::subcommands(parent);
    let mut all_subcommands = vec![];

    for (ref name, ref bin_name) in &subcommand_names {
        debug!(
            "get_subcommands_of:iter: parent={}, name={name}, bin_name={bin_name}",
            parent.get_name(),
        );
        let mut segments = vec![format!("({name})")];
        let subcommand_args = get_args_of(
            parser_of(parent, bin_name).expect(INTERNAL_ERROR_MSG),
            Some(parent),
        );

        if !subcommand_args.is_empty() {
            segments.push(subcommand_args);
        }

        // Get the help text of all child subcommands.
        let children = get_subcommands_of(parser_of(parent, bin_name).expect(INTERNAL_ERROR_MSG));

        if !children.is_empty() {
            segments.push(children);
        }

        segments.push(String::from(";;"));
        all_subcommands.push(segments.join("\n"));
    }

    let parent_bin_name = parent
        .get_bin_name()
        .expect("crate::generate should have set the bin_name");

    format!(
        "
    case $state in
    ({name})
        words=($line[{pos}] \"${{words[@]}}\")
        (( CURRENT += 1 ))
        curcontext=\"${{curcontext%:*:*}}:{name_hyphen}-command-$line[{pos}]:\"
        case $line[{pos}] in
            {subcommands}
        esac
    ;;
esac",
        name = parent.get_name(),
        name_hyphen = parent_bin_name.replace(' ', "-"),
        subcommands = all_subcommands.join("\n"),
        pos = parent.get_positionals().count() + 1
    )
}

// Get the Command for a given subcommand tree.
//
// Given the bin_name "a b c" and the Command for "a" this returns the "c" Command.
// Given the bin_name "a b c" and the Command for "b" this returns the "c" Command.
fn parser_of<'cmd>(parent: &'cmd Command, bin_name: &str) -> Option<&'cmd Command> {
    debug!("parser_of: p={}, bin_name={}", parent.get_name(), bin_name);

    if bin_name == parent.get_bin_name().unwrap_or_default() {
        return Some(parent);
    }

    for subcommand in parent.get_subcommands() {
        if let Some(ret) = parser_of(subcommand, bin_name) {
            return Some(ret);
        }
    }

    None
}

// Writes out the args section, which ends up being the flags, opts and positionals, and a jump to
// another ZSH function if there are subcommands.
// The structure works like this:
//    ([conflicting_args]) [multiple] arg [takes_value] [[help]] [: :(possible_values)]
//       ^-- list '-v -h'    ^--'*'          ^--'+'                   ^-- list 'one two three'
//
// An example from the rustup command:
//
// _arguments -C -s -S \
//         '(-h --help --verbose)-v[Enable verbose output]' \
//         '(-V -v --version --verbose --help)-h[Print help information]' \
//      # ... snip for brevity
//         ':: :_rustup_commands' \    # <-- displays subcommands
//         '*::: :->rustup' \          # <-- displays subcommand args and child subcommands
//     && ret=0
//
// The args used for _arguments are as follows:
//    -C: modify the $context internal variable
//    -s: Allow stacking of short args (i.e. -a -b -c => -abc)
//    -S: Do not complete anything after '--' and treat those as argument values
fn get_args_of(parent: &Command, p_global: Option<&Command>) -> String {
    debug!("get_args_of");

    let mut segments = vec![String::from("_arguments \"${_arguments_options[@]}\" : \\")];
    let opts = write_opts_of(parent, p_global);
    let flags = write_flags_of(parent, p_global);
    let positionals = write_positionals_of(parent);

    if !opts.is_empty() {
        segments.push(opts);
    }

    if !flags.is_empty() {
        segments.push(flags);
    }

    if !positionals.is_empty() {
        segments.push(positionals);
    }

    if parent.has_subcommands() {
        let parent_bin_name = parent
            .get_bin_name()
            .expect("crate::generate should have set the bin_name");
        let subcommand_bin_name = format!(
            "\":: :_{name}_commands\" \\",
            name = parent_bin_name.replace(' ', "__")
        );
        segments.push(subcommand_bin_name);

        let subcommand_text = format!("\"*::: :->{name}\" \\", name = parent.get_name());
        segments.push(subcommand_text);
    } else if parent.is_allow_external_subcommands_set() {
        // If the command has an external subcommand value parser, we need to
        // add a catch-all for the subcommand. Otherwise there would be no autocompletion whatsoever.
        segments.push(String::from("\"*::external_command:_default\" \\"));
    }

    segments.push(String::from("&& ret=0"));
    segments.join("\n")
}

// Uses either `possible_vals` or `value_hint` to give hints about possible argument values
fn value_completion(arg: &Arg) -> Option<String> {
    if let Some(values) = utils::possible_values(arg) {
        if values
            .iter()
            .any(|value| !value.is_hide_set() && value.get_help().is_some())
        {
            Some(format!(
                "(({}))",
                values
                    .iter()
                    .filter_map(|value| {
                        if value.is_hide_set() {
                            None
                        } else {
                            Some(format!(
                                r#"{name}\:"{tooltip}""#,
                                name = escape_value(value.get_name()),
                                tooltip =
                                    escape_help(&value.get_help().unwrap_or_default().to_string()),
                            ))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            ))
        } else {
            Some(format!(
                "({})",
                values
                    .iter()
                    .filter(|pv| !pv.is_hide_set())
                    .map(|n| n.get_name())
                    .collect::<Vec<_>>()
                    .join(" ")
            ))
        }
    } else {
        // NB! If you change this, please also update the table in `ValueHint` documentation.
        Some(
            match arg.get_value_hint() {
                ValueHint::Unknown => "_default",
                ValueHint::Other => "",
                ValueHint::AnyPath => "_files",
                ValueHint::FilePath => "_files",
                ValueHint::DirPath => "_files -/",
                ValueHint::ExecutablePath => "_absolute_command_paths",
                ValueHint::CommandName => "_command_names -e",
                ValueHint::CommandString => "_cmdstring",
                ValueHint::CommandWithArguments => "_cmdambivalent",
                ValueHint::Username => "_users",
                ValueHint::Hostname => "_hosts",
                ValueHint::Url => "_urls",
                ValueHint::EmailAddress => "_email_addresses",
                _ => {
                    return None;
                }
            }
            .to_string(),
        )
    }
}

/// Escape help string inside single quotes and brackets
fn escape_help(string: &str) -> String {
    string
        .replace('\\', "\\\\")
        .replace('\'', "'\\''")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(':', "\\:")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('\n', " ")
}

/// Escape value string inside single quotes and parentheses
fn escape_value(string: &str) -> String {
    string
        .replace('\\', "\\\\")
        .replace('\'', "'\\''")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(':', "\\:")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace(' ', "\\ ")
}

fn write_opts_of(p: &Command, p_global: Option<&Command>) -> String {
    debug!("write_opts_of");

    let mut ret = vec![];

    for o in p.get_opts() {
        debug!("write_opts_of:iter: o={}", o.get_id());

        let help = escape_help(&o.get_help().unwrap_or_default().to_string());
        let conflicts = arg_conflicts(p, o, p_global);

        let multiple = if let ArgAction::Count | ArgAction::Append = o.get_action() {
            "*"
        } else {
            ""
        };

        let vn = match o.get_value_names() {
            None => " ".to_string(),
            Some(val) => val[0].to_string(),
        };
        let vc = match value_completion(o) {
            Some(val) => format!(":{vn}:{val}"),
            None => format!(":{vn}: "),
        };
        let vc = vc.repeat(o.get_num_args().expect("built").min_values());

        if let Some(shorts) = o.get_short_and_visible_aliases() {
            for short in shorts {
                let s = format!("'{conflicts}{multiple}-{short}+[{help}]{vc}' \\");

                debug!("write_opts_of:iter: Wrote...{}", &*s);
                ret.push(s);
            }
        }
        if let Some(longs) = o.get_long_and_visible_aliases() {
            for long in longs {
                let l = format!("'{conflicts}{multiple}--{long}=[{help}]{vc}' \\");

                debug!("write_opts_of:iter: Wrote...{}", &*l);
                ret.push(l);
            }
        }
    }

    ret.join("\n")
}

fn arg_conflicts(cmd: &Command, arg: &Arg, app_global: Option<&Command>) -> String {
    fn push_conflicts(conflicts: &[&Arg], res: &mut Vec<String>) {
        for conflict in conflicts {
            if let Some(s) = conflict.get_short() {
                res.push(format!("-{s}"));
            }

            if let Some(l) = conflict.get_long() {
                res.push(format!("--{l}"));
            }
        }
    }

    let mut res = vec![];
    match (app_global, arg.is_global_set()) {
        (Some(x), true) => {
            let conflicts = x.get_arg_conflicts_with(arg);

            if conflicts.is_empty() {
                return String::new();
            }

            push_conflicts(&conflicts, &mut res);
        }
        (_, _) => {
            let conflicts = cmd.get_arg_conflicts_with(arg);

            if conflicts.is_empty() {
                return String::new();
            }

            push_conflicts(&conflicts, &mut res);
        }
    };

    format!("({})", res.join(" "))
}

fn write_flags_of(p: &Command, p_global: Option<&Command>) -> String {
    debug!("write_flags_of;");

    let mut ret = vec![];

    for f in utils::flags(p) {
        debug!("write_flags_of:iter: f={}", f.get_id());

        let help = escape_help(&f.get_help().unwrap_or_default().to_string());
        let conflicts = arg_conflicts(p, &f, p_global);

        let multiple = if let ArgAction::Count | ArgAction::Append = f.get_action() {
            "*"
        } else {
            ""
        };

        if let Some(short) = f.get_short() {
            let s = format!("'{conflicts}{multiple}-{short}[{help}]' \\");

            debug!("write_flags_of:iter: Wrote...{}", &*s);

            ret.push(s);

            if let Some(short_aliases) = f.get_visible_short_aliases() {
                for alias in short_aliases {
                    let s = format!("'{conflicts}{multiple}-{alias}[{help}]' \\",);

                    debug!("write_flags_of:iter: Wrote...{}", &*s);

                    ret.push(s);
                }
            }
        }

        if let Some(long) = f.get_long() {
            let l = format!("'{conflicts}{multiple}--{long}[{help}]' \\");

            debug!("write_flags_of:iter: Wrote...{}", &*l);

            ret.push(l);

            if let Some(aliases) = f.get_visible_aliases() {
                for alias in aliases {
                    let l = format!("'{conflicts}{multiple}--{alias}[{help}]' \\");

                    debug!("write_flags_of:iter: Wrote...{}", &*l);

                    ret.push(l);
                }
            }
        }
    }

    ret.join("\n")
}

fn write_positionals_of(p: &Command) -> String {
    debug!("write_positionals_of;");

    let mut ret = vec![];

    // Completions for commands that end with two Vec arguments require special care.
    // - You can have two Vec args separated with a custom value terminator.
    // - You can have two Vec args with the second one set to last (raw sets last)
    //   which will require a '--' separator to be used before the second argument
    //   on the command-line.
    //
    // We use the '-S' _arguments option to disable completion after '--'. Thus, the
    // completion for the second argument in scenario (B) does not need to be emitted
    // because it is implicitly handled by the '-S' option.
    // We only need to emit the first catch-all.
    //
    // Have we already emitted a catch-all multi-valued positional argument
    // without a custom value terminator?
    let mut catch_all_emitted = false;

    for arg in p.get_positionals() {
        debug!("write_positionals_of:iter: arg={}", arg.get_id());

        let num_args = arg.get_num_args().expect("built");
        let is_multi_valued = num_args.max_values() > 1;

        if catch_all_emitted && (arg.is_last_set() || is_multi_valued) {
            // This is the final argument and it also takes multiple arguments.
            // We've already emitted a catch-all positional argument so we don't need
            // to emit anything for this argument because it is implicitly handled by
            // the use of the '-S' _arguments option.
            continue;
        }

        let cardinality_value;
        // If we have any subcommands, we'll emit a catch-all argument, so we shouldn't
        // emit one here.
        let cardinality = if is_multi_valued && !p.has_subcommands() {
            match arg.get_value_terminator() {
                Some(terminator) => {
                    cardinality_value = format!("*{}:", escape_value(terminator));
                    cardinality_value.as_str()
                }
                None => {
                    catch_all_emitted = true;
                    "*:"
                }
            }
        } else if !arg.is_required_set() {
            ":"
        } else {
            ""
        };

        let a = format!(
            "'{cardinality}:{name}{help}:{value_completion}' \\",
            cardinality = cardinality,
            name = arg.get_id(),
            help = arg
                .get_help()
                .map(|s| s.to_string())
                .map(|v| " -- ".to_owned() + &v)
                .unwrap_or_else(|| "".to_owned())
                .replace('[', "\\[")
                .replace(']', "\\]")
                .replace('\'', "'\\''")
                .replace(':', "\\:"),
            value_completion = value_completion(arg).unwrap_or_default()
        );

        debug!("write_positionals_of:iter: Wrote...{a}");

        ret.push(a);
    }

    ret.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{escape_help, escape_value};

    #[test]
    fn test_escape_value() {
        let raw_string = "\\ [foo]() `bar https://$PATH";
        assert_eq!(
            escape_value(raw_string),
            "\\\\\\ \\[foo\\]\\(\\)\\ \\`bar\\ https\\://\\$PATH"
        );
    }

    #[test]
    fn test_escape_help() {
        let raw_string = "\\ [foo]() `bar https://$PATH";
        assert_eq!(
            escape_help(raw_string),
            "\\\\ \\[foo\\]() \\`bar https\\://\\$PATH"
        );
    }
}
