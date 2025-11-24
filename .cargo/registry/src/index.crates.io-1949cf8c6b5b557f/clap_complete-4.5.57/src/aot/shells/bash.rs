use std::{
    fmt::Write as _,
    io::{Error, Write},
};

use clap::{Arg, Command, ValueHint};

use crate::generator::{utils, Generator};

/// Generate bash completion file
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Bash;

impl Generator for Bash {
    fn file_name(&self, name: &str) -> String {
        format!("{name}.bash")
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn Write) {
        self.try_generate(cmd, buf)
            .expect("failed to write completion file");
    }

    fn try_generate(&self, cmd: &Command, buf: &mut dyn Write) -> Result<(), Error> {
        let bin_name = cmd
            .get_bin_name()
            .expect("crate::generate should have set the bin_name");

        let fn_name = bin_name.replace('-', "__");

        write!(
            buf,
            "_{name}() {{
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ \"${{BASH_VERSINFO[0]}}\" -ge 4 ]]; then
        cur=\"$2\"
    else
        cur=\"${{COMP_WORDS[COMP_CWORD]}}\"
    fi
    prev=\"$3\"
    cmd=\"\"
    opts=\"\"

    for i in \"${{COMP_WORDS[@]:0:COMP_CWORD}}\"
    do
        case \"${{cmd}},${{i}}\" in
            \",$1\")
                cmd=\"{cmd}\"
                ;;{subcmds}
            *)
                ;;
        esac
    done

    case \"${{cmd}}\" in
        {cmd})
            opts=\"{name_opts}\"
            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )
                return 0
            fi
            case \"${{prev}}\" in{name_opts_details}
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )
            return 0
            ;;{subcmd_details}
    esac
}}

if [[ \"${{BASH_VERSINFO[0]}}\" -eq 4 && \"${{BASH_VERSINFO[1]}}\" -ge 4 || \"${{BASH_VERSINFO[0]}}\" -gt 4 ]]; then
    complete -F _{name} -o nosort -o bashdefault -o default {name}
else
    complete -F _{name} -o bashdefault -o default {name}
fi
",
            name = bin_name,
            cmd = fn_name,
            name_opts = all_options_for_path(cmd, bin_name),
            name_opts_details = option_details_for_path(cmd, bin_name),
            subcmds = all_subcommands(cmd, &fn_name),
            subcmd_details = subcommand_details(cmd)
        )
    }
}

fn all_subcommands(cmd: &Command, parent_fn_name: &str) -> String {
    debug!("all_subcommands");

    fn add_command(
        parent_fn_name: &str,
        cmd: &Command,
        subcmds: &mut Vec<(String, String, String)>,
    ) {
        let fn_name = format!(
            "{parent_fn_name}__{cmd_name}",
            parent_fn_name = parent_fn_name,
            cmd_name = cmd.get_name().to_string().replace('-', "__")
        );
        subcmds.push((
            parent_fn_name.to_string(),
            cmd.get_name().to_string(),
            fn_name.clone(),
        ));
        for alias in cmd.get_visible_aliases() {
            subcmds.push((
                parent_fn_name.to_string(),
                alias.to_string(),
                fn_name.clone(),
            ));
        }
        for subcmd in cmd.get_subcommands() {
            add_command(&fn_name, subcmd, subcmds);
        }
    }
    let mut subcmds = vec![];
    for subcmd in cmd.get_subcommands() {
        add_command(parent_fn_name, subcmd, &mut subcmds);
    }
    subcmds.sort();

    let mut cases = vec![String::new()];
    for (parent_fn_name, name, fn_name) in subcmds {
        cases.push(format!(
            "{parent_fn_name},{name})
                cmd=\"{fn_name}\"
                ;;",
        ));
    }

    cases.join("\n            ")
}

fn subcommand_details(cmd: &Command) -> String {
    debug!("subcommand_details");

    let mut subcmd_dets = vec![String::new()];
    let mut scs = utils::all_subcommands(cmd)
        .iter()
        .map(|x| x.1.replace(' ', "__"))
        .collect::<Vec<_>>();

    scs.sort();
    scs.dedup();

    subcmd_dets.extend(scs.iter().map(|sc| {
        format!(
            "{subcmd})
            opts=\"{sc_opts}\"
            if [[ ${{cur}} == -* || ${{COMP_CWORD}} -eq {level} ]] ; then
                COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )
                return 0
            fi
            case \"${{prev}}\" in{opts_details}
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W \"${{opts}}\" -- \"${{cur}}\") )
            return 0
            ;;",
            subcmd = sc.replace('-', "__"),
            sc_opts = all_options_for_path(cmd, sc),
            level = sc.split("__").map(|_| 1).sum::<u64>(),
            opts_details = option_details_for_path(cmd, sc)
        )
    }));

    subcmd_dets.join("\n        ")
}

fn option_details_for_path(cmd: &Command, path: &str) -> String {
    debug!("option_details_for_path: path={path}");

    let p = utils::find_subcommand_with_path(cmd, path.split("__").skip(1).collect());
    let mut opts = vec![String::new()];

    for o in p.get_opts() {
        let compopt = match o.get_value_hint() {
            ValueHint::FilePath => Some("compopt -o filenames"),
            ValueHint::DirPath => Some("compopt -o plusdirs"),
            ValueHint::Other => Some("compopt -o nospace"),
            _ => None,
        };

        if let Some(longs) = o.get_long_and_visible_aliases() {
            opts.extend(longs.iter().map(|long| {
                let mut v = vec![format!("--{})", long)];

                if o.get_value_hint() == ValueHint::FilePath {
                    v.extend([
                        "local oldifs".to_string(),
                        r#"if [ -n "${IFS+x}" ]; then"#.to_string(),
                        r#"    oldifs="$IFS""#.to_string(),
                        "fi".to_string(),
                        r#"IFS=$'\n'"#.to_string(),
                        format!("COMPREPLY=({})", vals_for(o)),
                        r#"if [ -n "${oldifs+x}" ]; then"#.to_string(),
                        r#"    IFS="$oldifs""#.to_string(),
                        "fi".to_string(),
                    ]);
                } else {
                    v.push(format!("COMPREPLY=({})", vals_for(o)));
                }

                if let Some(copt) = compopt {
                    v.extend([
                        r#"if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then"#.to_string(),
                        format!("    {copt}"),
                        "fi".to_string(),
                    ]);
                }

                v.extend(["return 0", ";;"].iter().map(|s| (*s).to_string()));
                v.join("\n                    ")
            }));
        }

        if let Some(shorts) = o.get_short_and_visible_aliases() {
            opts.extend(shorts.iter().map(|short| {
                let mut v = vec![format!("-{})", short)];

                if o.get_value_hint() == ValueHint::FilePath {
                    v.extend([
                        "local oldifs".to_string(),
                        r#"if [ -n "${IFS+x}" ]; then"#.to_string(),
                        r#"    oldifs="$IFS""#.to_string(),
                        "fi".to_string(),
                        r#"IFS=$'\n'"#.to_string(),
                        format!("COMPREPLY=({})", vals_for(o)),
                        r#"if [ -n "${oldifs+x}" ]; then"#.to_string(),
                        r#"    IFS="$oldifs""#.to_string(),
                        "fi".to_string(),
                    ]);
                } else {
                    v.push(format!("COMPREPLY=({})", vals_for(o)));
                }

                if let Some(copt) = compopt {
                    v.extend([
                        r#"if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then"#.to_string(),
                        format!("    {copt}"),
                        "fi".to_string(),
                    ]);
                }

                v.extend(["return 0", ";;"].iter().map(|s| (*s).to_string()));
                v.join("\n                    ")
            }));
        }
    }

    opts.join("\n                ")
}

fn vals_for(o: &Arg) -> String {
    debug!("vals_for: o={}", o.get_id());

    if let Some(vals) = utils::possible_values(o) {
        format!(
            "$(compgen -W \"{}\" -- \"${{cur}}\")",
            vals.iter()
                .filter(|pv| !pv.is_hide_set())
                .map(|n| n.get_name())
                .collect::<Vec<_>>()
                .join(" ")
        )
    } else if o.get_value_hint() == ValueHint::DirPath {
        String::from("") // should be empty to avoid duplicate candidates
    } else if o.get_value_hint() == ValueHint::Other {
        String::from("\"${cur}\"")
    } else {
        String::from("$(compgen -f \"${cur}\")")
    }
}

fn all_options_for_path(cmd: &Command, path: &str) -> String {
    debug!("all_options_for_path: path={path}");

    let p = utils::find_subcommand_with_path(cmd, path.split("__").skip(1).collect());

    let mut opts = String::new();
    for short in utils::shorts_and_visible_aliases(p) {
        write!(&mut opts, "-{short} ").expect("writing to String is infallible");
    }
    for long in utils::longs_and_visible_aliases(p) {
        write!(&mut opts, "--{long} ").expect("writing to String is infallible");
    }
    for pos in p.get_positionals() {
        if let Some(vals) = utils::possible_values(pos) {
            for value in vals {
                write!(&mut opts, "{} ", value.get_name())
                    .expect("writing to String is infallible");
            }
        } else {
            write!(&mut opts, "{pos} ").expect("writing to String is infallible");
        }
    }
    for (sc, _) in utils::subcommands(p) {
        write!(&mut opts, "{sc} ").expect("writing to String is infallible");
    }
    opts.pop();

    opts
}
