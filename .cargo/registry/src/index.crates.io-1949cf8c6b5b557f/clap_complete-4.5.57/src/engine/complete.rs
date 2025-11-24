use std::ffi::OsStr;
use std::ffi::OsString;

use clap_lex::OsStrExt as _;

use super::custom::complete_path;
use super::ArgValueCandidates;
use super::ArgValueCompleter;
use super::CompletionCandidate;
use super::SubcommandCandidates;

/// Complete the given command, shell-agnostic
pub fn complete(
    cmd: &mut clap::Command,
    args: Vec<OsString>,
    arg_index: usize,
    current_dir: Option<&std::path::Path>,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
    debug!("complete: args={args:?}, arg_index={arg_index:?}, current_dir={current_dir:?}");
    cmd.build();

    let raw_args = clap_lex::RawArgs::new(args);
    let mut cursor = raw_args.cursor();
    let mut target_cursor = raw_args.cursor();
    raw_args.seek(
        &mut target_cursor,
        clap_lex::SeekFrom::Start(arg_index as u64),
    );
    // As we loop, `cursor` will always be pointing to the next item
    raw_args.next_os(&mut target_cursor);
    debug!("complete: target_cursor={target_cursor:?}");

    // TODO: Multicall support
    if !cmd.is_no_binary_name_set() {
        raw_args.next_os(&mut cursor);
    }

    let mut current_cmd = &*cmd;
    let mut pos_index = 1;
    let mut is_escaped = false;
    let mut next_state = ParseState::ValueDone;
    while let Some(arg) = raw_args.next(&mut cursor) {
        let current_state = next_state;
        next_state = ParseState::ValueDone;
        debug!(
            "complete::next: arg={:?}, current_state={current_state:?}, cursor={cursor:?}",
            arg.to_value_os(),
        );
        if cursor == target_cursor {
            return complete_arg(&arg, current_cmd, current_dir, pos_index, current_state);
        }

        if let Ok(value) = arg.to_value() {
            if let Some(next_cmd) = current_cmd.find_subcommand(value) {
                current_cmd = next_cmd;
                pos_index = 1;
                continue;
            }
        }

        if is_escaped {
            (next_state, pos_index) =
                parse_positional(current_cmd, pos_index, is_escaped, current_state);
        } else if arg.is_escape() {
            is_escaped = true;
        } else if opt_allows_hyphen(&current_state, &arg) {
            match current_state {
                ParseState::Opt((opt, count)) => next_state = parse_opt_value(opt, count),
                _ => unreachable!("else branch is only reachable in Opt state"),
            }
        } else if let Some((flag, value)) = arg.to_long() {
            if let Ok(flag) = flag {
                let opt = current_cmd.get_arguments().find(|a| {
                    let longs = a.get_long_and_visible_aliases();
                    let is_find = longs.map(|v| {
                        let mut iter = v.into_iter();
                        let s = iter.find(|s| *s == flag);
                        s.is_some()
                    });
                    is_find.unwrap_or(false)
                });

                if let Some(opt) = opt {
                    if opt.get_num_args().expect("built").takes_values() && value.is_none() {
                        next_state = ParseState::Opt((opt, 1));
                    };
                } else if pos_allows_hyphen(current_cmd, pos_index) {
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, current_state);
                }
            }
        } else if let Some(short) = arg.to_short() {
            let (_, takes_value_opt, mut short) = parse_shortflags(current_cmd, short);
            if let Some(opt) = takes_value_opt {
                if short.next_value_os().is_none() {
                    next_state = ParseState::Opt((opt, 1));
                }
            } else if pos_allows_hyphen(current_cmd, pos_index) {
                (next_state, pos_index) =
                    parse_positional(current_cmd, pos_index, is_escaped, current_state);
            }
        } else {
            match current_state {
                ParseState::ValueDone | ParseState::Pos(..) => {
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, current_state);
                }
                ParseState::Opt((opt, count)) => next_state = parse_opt_value(opt, count),
            }
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "no completion generated",
    ))
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum ParseState<'a> {
    /// Parsing a value done, there is no state to record.
    ValueDone,

    /// Parsing a positional argument after `--`. `Pos(pos_index`, `takes_num_args`)
    Pos((usize, usize)),

    /// Parsing a optional flag argument
    Opt((&'a clap::Arg, usize)),
}

fn complete_arg(
    arg: &clap_lex::ParsedArg<'_>,
    cmd: &clap::Command,
    current_dir: Option<&std::path::Path>,
    pos_index: usize,
    state: ParseState<'_>,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
    debug!(
        "complete_arg: arg={:?}, cmd={:?}, current_dir={:?}, pos_index={:?}, state={:?}",
        arg,
        cmd.get_name(),
        current_dir,
        pos_index,
        state
    );
    let mut completions = Vec::<CompletionCandidate>::new();

    match state {
        ParseState::ValueDone => {
            if let Ok(value) = arg.to_value() {
                completions.extend(complete_subcommand(value, cmd));
            }

            if let Some(positional) = cmd
                .get_positionals()
                .find(|p| p.get_index() == Some(pos_index))
            {
                completions.extend(complete_arg_value(arg.to_value(), positional, current_dir));
            }
            completions.extend(complete_option(arg, cmd, current_dir));
        }
        ParseState::Pos((_, num_arg)) => {
            if let Some(positional) = cmd
                .get_positionals()
                .find(|p| p.get_index() == Some(pos_index))
            {
                completions.extend(complete_arg_value(arg.to_value(), positional, current_dir));
                if positional
                    .get_num_args()
                    .is_some_and(|num_args| num_arg >= num_args.min_values())
                {
                    completions.extend(complete_option(arg, cmd, current_dir));
                }
            }
        }
        ParseState::Opt((opt, count)) => {
            completions.extend(complete_arg_value(arg.to_value(), opt, current_dir));
            let min = opt.get_num_args().map(|r| r.min_values()).unwrap_or(0);
            if count > min {
                // Also complete this raw_arg as a positional argument, flags, options and subcommand.
                completions.extend(complete_arg(
                    arg,
                    cmd,
                    current_dir,
                    pos_index,
                    ParseState::ValueDone,
                )?);
            }
        }
    }
    if completions.iter().any(|a| !a.is_hide_set()) {
        completions.retain(|a| !a.is_hide_set());
    }
    let mut seen_ids = std::collections::HashSet::new();
    completions.retain(move |a| {
        if let Some(id) = a.get_id().cloned() {
            seen_ids.insert(id)
        } else {
            true
        }
    });

    let mut tags = Vec::new();
    for candidate in &completions {
        let tag = candidate.get_tag().cloned();
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    completions.sort_by_key(|c| {
        (
            tags.iter().position(|t| c.get_tag() == t.as_ref()),
            c.get_display_order(),
        )
    });

    Ok(completions)
}

fn complete_option(
    arg: &clap_lex::ParsedArg<'_>,
    cmd: &clap::Command,
    current_dir: Option<&std::path::Path>,
) -> Vec<CompletionCandidate> {
    let mut completions = Vec::<CompletionCandidate>::new();
    if arg.is_empty() {
        completions.extend(longs_and_visible_aliases(cmd));
        completions.extend(hidden_longs_aliases(cmd));

        let dash_or_arg = if arg.is_empty() {
            "-".into()
        } else {
            arg.to_value_os().to_string_lossy()
        };
        completions.extend(
            shorts_and_visible_aliases(cmd)
                .into_iter()
                .map(|comp| comp.add_prefix(dash_or_arg.to_string())),
        );
    } else if arg.is_stdio() {
        // HACK: Assuming knowledge of is_stdio
        let dash_or_arg = if arg.is_empty() {
            "-".into()
        } else {
            arg.to_value_os().to_string_lossy()
        };
        completions.extend(
            shorts_and_visible_aliases(cmd)
                .into_iter()
                .map(|comp| comp.add_prefix(dash_or_arg.to_string())),
        );

        completions.extend(longs_and_visible_aliases(cmd));
        completions.extend(hidden_longs_aliases(cmd));
    } else if arg.is_escape() {
        // HACK: Assuming knowledge of is_escape
        completions.extend(longs_and_visible_aliases(cmd));
        completions.extend(hidden_longs_aliases(cmd));
    } else if let Some((flag, value)) = arg.to_long() {
        if let Ok(flag) = flag {
            if let Some(value) = value {
                if let Some(arg) = cmd.get_arguments().find(|a| a.get_long() == Some(flag)) {
                    completions.extend(
                        complete_arg_value(value.to_str().ok_or(value), arg, current_dir)
                            .into_iter()
                            .map(|comp| comp.add_prefix(format!("--{flag}="))),
                    );
                }
            } else {
                completions.extend(
                    longs_and_visible_aliases(cmd)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
                completions.extend(
                    hidden_longs_aliases(cmd)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
            }
        }
    } else if let Some(short) = arg.to_short() {
        if !short.is_negative_number() {
            // Find the first takes_values option.
            let (leading_flags, takes_value_opt, mut short) = parse_shortflags(cmd, short);

            // Clone `short` to `peek_short` to peek whether the next flag is a `=`.
            if let Some(opt) = takes_value_opt {
                let mut peek_short = short.clone();
                let has_equal = if let Some(Ok('=')) = peek_short.next_flag() {
                    short.next_flag();
                    true
                } else {
                    false
                };

                let value = short.next_value_os().unwrap_or(OsStr::new(""));
                completions.extend(
                    complete_arg_value(value.to_str().ok_or(value), opt, current_dir)
                        .into_iter()
                        .map(|comp| {
                            let sep = if has_equal { "=" } else { "" };
                            comp.add_prefix(format!("-{leading_flags}{sep}"))
                        }),
                );
            } else {
                completions.extend(
                    shorts_and_visible_aliases(cmd)
                        .into_iter()
                        .map(|comp| comp.add_prefix(format!("-{leading_flags}"))),
                );
            }
        }
    }
    completions
}

fn complete_arg_value(
    value: Result<&str, &OsStr>,
    arg: &clap::Arg,
    current_dir: Option<&std::path::Path>,
) -> Vec<CompletionCandidate> {
    let mut values = Vec::new();
    debug!("complete_arg_value: arg={arg:?}, value={value:?}");

    let (prefix, value) =
        rsplit_delimiter(value, arg.get_value_delimiter()).unwrap_or((None, value));

    let value_os = match value {
        Ok(value) => OsStr::new(value),
        Err(value_os) => value_os,
    };

    if let Some(completer) = arg.get::<ArgValueCompleter>() {
        values.extend(completer.complete(value_os));
    } else if let Some(completer) = arg.get::<ArgValueCandidates>() {
        values.extend(complete_custom_arg_value(value_os, completer));
    } else if let Some(possible_values) = possible_values(arg) {
        if let Ok(value) = value {
            values.extend(possible_values.into_iter().filter_map(|p| {
                let name = p.get_name();
                name.starts_with(value).then(|| {
                    CompletionCandidate::new(OsString::from(name))
                        .help(p.get_help().cloned())
                        .hide(p.is_hide_set())
                })
            }));
        }
    } else {
        match arg.get_value_hint() {
            clap::ValueHint::Unknown | clap::ValueHint::Other => {
                // Should not complete
            }
            clap::ValueHint::AnyPath => {
                values.extend(complete_path(value_os, current_dir, &|_| true));
            }
            clap::ValueHint::FilePath => {
                values.extend(complete_path(value_os, current_dir, &|p| p.is_file()));
            }
            clap::ValueHint::DirPath => {
                values.extend(complete_path(value_os, current_dir, &|p| p.is_dir()));
            }
            clap::ValueHint::ExecutablePath => {
                use is_executable::IsExecutable;
                values.extend(complete_path(value_os, current_dir, &|p| p.is_executable()));
            }
            clap::ValueHint::CommandName
            | clap::ValueHint::CommandString
            | clap::ValueHint::CommandWithArguments
            | clap::ValueHint::Username
            | clap::ValueHint::Hostname
            | clap::ValueHint::Url
            | clap::ValueHint::EmailAddress => {
                // No completion implementation
            }
            _ => {
                // Safe-ish fallback
                values.extend(complete_path(value_os, current_dir, &|_| true));
            }
        }

        values.sort();
    }

    if let Some(prefix) = prefix {
        values = values
            .into_iter()
            .map(|comp| comp.add_prefix(prefix))
            .collect();
    }
    values = values
        .into_iter()
        .map(|comp| {
            if comp.get_tag().is_some() {
                comp
            } else {
                comp.tag(Some(arg.to_string().into()))
            }
        })
        .collect();

    values
}

fn rsplit_delimiter<'s, 'o>(
    value: Result<&'s str, &'o OsStr>,
    delimiter: Option<char>,
) -> Option<(Option<&'s str>, Result<&'s str, &'o OsStr>)> {
    let delimiter = delimiter?;
    let value = value.ok()?;
    let pos = value.rfind(delimiter)?;
    let (prefix, value) = value.split_at(pos + delimiter.len_utf8());
    Some((Some(prefix), Ok(value)))
}

fn complete_custom_arg_value(
    value: &OsStr,
    completer: &ArgValueCandidates,
) -> Vec<CompletionCandidate> {
    debug!("complete_custom_arg_value: completer={completer:?}, value={value:?}");

    let mut values = completer.candidates();
    values.retain(|comp| comp.get_value().starts_with(&value.to_string_lossy()));
    values
}

fn complete_subcommand(value: &str, cmd: &clap::Command) -> Vec<CompletionCandidate> {
    debug!(
        "complete_subcommand: cmd={:?}, value={:?}",
        cmd.get_name(),
        value
    );

    let mut scs: Vec<CompletionCandidate> = subcommands(cmd)
        .into_iter()
        .filter(|x| x.get_value().starts_with(value))
        .collect();
    if cmd.is_allow_external_subcommands_set() {
        let external_completer = cmd.get::<SubcommandCandidates>();
        if let Some(completer) = external_completer {
            scs.extend(complete_external_subcommand(value, completer));
        }
    }

    scs.sort();
    scs.dedup();
    scs
}

fn complete_external_subcommand(
    value: &str,
    completer: &SubcommandCandidates,
) -> Vec<CompletionCandidate> {
    debug!("complete_custom_arg_value: completer={completer:?}, value={value:?}");

    let mut values = Vec::new();
    let custom_arg_values = completer.candidates();
    values.extend(custom_arg_values);

    values.retain(|comp| comp.get_value().starts_with(value));

    values
}

/// Gets all the long options, their visible aliases and flags of a [`clap::Command`] with formatted `--` prefix.
/// Includes `help` and `version` depending on the [`clap::Command`] settings.
fn longs_and_visible_aliases(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("longs: name={}", p.get_name());

    p.get_arguments()
        .filter_map(|a| {
            a.get_long_and_visible_aliases().map(|longs| {
                longs
                    .into_iter()
                    .map(|s| populate_arg_candidate(CompletionCandidate::new(format!("--{s}")), a))
            })
        })
        .flatten()
        .collect()
}

/// Gets all the long hidden aliases and flags of a [`clap::Command`].
fn hidden_longs_aliases(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("longs: name={}", p.get_name());

    p.get_arguments()
        .filter_map(|a| {
            a.get_aliases().map(|longs| {
                longs.into_iter().map(|s| {
                    populate_arg_candidate(CompletionCandidate::new(format!("--{s}")), a).hide(true)
                })
            })
        })
        .flatten()
        .collect()
}

/// Gets all the short options, their visible aliases and flags of a [`clap::Command`].
/// Includes `h` and `V` depending on the [`clap::Command`] settings.
fn shorts_and_visible_aliases(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("shorts: name={}", p.get_name());

    p.get_arguments()
        .filter_map(|a| {
            a.get_short_and_visible_aliases().map(|shorts| {
                shorts.into_iter().map(|s| {
                    populate_arg_candidate(CompletionCandidate::new(s.to_string()), a).help(
                        a.get_help()
                            .cloned()
                            .or_else(|| a.get_long().map(|long| format!("--{long}").into())),
                    )
                })
            })
        })
        .flatten()
        .collect()
}

fn populate_arg_candidate(candidate: CompletionCandidate, arg: &clap::Arg) -> CompletionCandidate {
    candidate
        .help(arg.get_help().cloned())
        .id(Some(format!("arg::{}", arg.get_id())))
        .tag(Some(
            arg.get_help_heading()
                .unwrap_or("Options")
                .to_owned()
                .into(),
        ))
        .display_order(Some(arg.get_display_order()))
        .hide(arg.is_hide_set())
}

/// Get the possible values for completion
fn possible_values(a: &clap::Arg) -> Option<Vec<clap::builder::PossibleValue>> {
    if !a.get_num_args().expect("built").takes_values() {
        None
    } else {
        a.get_value_parser()
            .possible_values()
            .map(|pvs| pvs.collect())
    }
}

/// Gets subcommands of [`clap::Command`] in the form of `("name", "bin_name")`.
///
/// Subcommand `rustup toolchain install` would be converted to
/// `("install", "rustup toolchain install")`.
fn subcommands(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("subcommands: name={}", p.get_name());
    debug!("subcommands: Has subcommands...{:?}", p.has_subcommands());
    p.get_subcommands()
        .flat_map(|sc| {
            sc.get_name_and_visible_aliases()
                .into_iter()
                .map(|s| populate_command_candidate(CompletionCandidate::new(s.to_string()), p, sc))
                .chain(sc.get_aliases().map(|s| {
                    populate_command_candidate(CompletionCandidate::new(s.to_string()), p, sc)
                        .hide(true)
                }))
        })
        .collect()
}

fn populate_command_candidate(
    candidate: CompletionCandidate,
    cmd: &clap::Command,
    subcommand: &clap::Command,
) -> CompletionCandidate {
    candidate
        .help(subcommand.get_about().cloned())
        .id(Some(format!("command::{}", subcommand.get_name())))
        .tag(Some(
            cmd.get_subcommand_help_heading()
                .unwrap_or("Commands")
                .to_owned()
                .into(),
        ))
        .display_order(Some(subcommand.get_display_order()))
        .hide(subcommand.is_hide_set())
}

/// Parse the short flags and find the first `takes_values` option.
fn parse_shortflags<'c, 's>(
    cmd: &'c clap::Command,
    mut short: clap_lex::ShortFlags<'s>,
) -> (String, Option<&'c clap::Arg>, clap_lex::ShortFlags<'s>) {
    let takes_value_opt;
    let mut leading_flags = String::new();
    // Find the first takes_values option.
    loop {
        match short.next_flag() {
            Some(Ok(opt)) => {
                leading_flags.push(opt);
                let opt = cmd.get_arguments().find(|a| {
                    let shorts = a.get_short_and_visible_aliases();
                    let is_find = shorts.map(|v| {
                        let mut iter = v.into_iter();
                        let c = iter.find(|c| *c == opt);
                        c.is_some()
                    });
                    is_find.unwrap_or(false)
                });
                if opt
                    .map(|o| o.get_num_args().expect("built").takes_values())
                    .unwrap_or(false)
                {
                    takes_value_opt = opt;
                    break;
                }
            }
            Some(Err(_)) | None => {
                takes_value_opt = None;
                break;
            }
        }
    }

    (leading_flags, takes_value_opt, short)
}

/// Parse the positional arguments. Return the new state and the new positional index.
fn parse_positional<'a>(
    cmd: &clap::Command,
    pos_index: usize,
    is_escaped: bool,
    state: ParseState<'a>,
) -> (ParseState<'a>, usize) {
    let pos_arg = cmd
        .get_positionals()
        .find(|p| p.get_index() == Some(pos_index));
    let num_args = pos_arg
        .and_then(|a| a.get_num_args().map(|r| r.max_values()))
        .unwrap_or(1);

    let update_state_with_new_positional = |pos_index| -> (ParseState<'a>, usize) {
        if num_args > 1 {
            (ParseState::Pos((pos_index, 1)), pos_index)
        } else {
            if is_escaped {
                (ParseState::Pos((pos_index, 1)), pos_index + 1)
            } else {
                (ParseState::ValueDone, pos_index + 1)
            }
        }
    };
    match state {
        ParseState::ValueDone => {
            update_state_with_new_positional(pos_index)
        },
        ParseState::Pos((prev_pos_index, num_arg)) => {
            if prev_pos_index == pos_index {
                if num_arg + 1 < num_args {
                    (ParseState::Pos((pos_index, num_arg + 1)), pos_index)
                } else {
                    if is_escaped {
                        (ParseState::Pos((pos_index, 1)), pos_index + 1)
                    } else {
                        (ParseState::ValueDone, pos_index + 1)
                    }
                }
            } else {
                update_state_with_new_positional(pos_index)
            }
        }
        ParseState::Opt(..) => unreachable!(
            "This branch won't be hit,
            because ParseState::Opt should not be seen as a positional argument and passed to this function."
        ),
    }
}

/// Parse optional flag argument. Return new state
fn parse_opt_value(opt: &clap::Arg, count: usize) -> ParseState<'_> {
    let range = opt.get_num_args().expect("built");
    let max = range.max_values();
    if count < max {
        ParseState::Opt((opt, count + 1))
    } else {
        ParseState::ValueDone
    }
}

fn pos_allows_hyphen(cmd: &clap::Command, pos_index: usize) -> bool {
    cmd.get_positionals()
        .find(|a| a.get_index() == Some(pos_index))
        .map(|p| p.is_allow_hyphen_values_set())
        .unwrap_or(false)
}

fn opt_allows_hyphen(state: &ParseState<'_>, arg: &clap_lex::ParsedArg<'_>) -> bool {
    let val = arg.to_value_os();
    if val.starts_with("-") {
        if let ParseState::Opt((opt, _)) = state {
            return opt.is_allow_hyphen_values_set();
        }
    }

    false
}
