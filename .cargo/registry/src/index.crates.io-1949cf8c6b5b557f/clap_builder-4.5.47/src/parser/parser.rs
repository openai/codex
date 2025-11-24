// Std
use std::{
    cell::Cell,
    ffi::{OsStr, OsString},
};

use clap_lex::OsStrExt as _;

// Internal
use crate::builder::{Arg, Command};
use crate::error::Error as ClapError;
use crate::error::Result as ClapResult;
use crate::mkeymap::KeyType;
use crate::output::Usage;
use crate::parser::features::suggestions;
use crate::parser::{ArgMatcher, SubCommand};
use crate::parser::{Validator, ValueSource};
use crate::util::AnyValue;
use crate::util::Id;
use crate::ArgAction;
use crate::INTERNAL_ERROR_MSG;

pub(crate) struct Parser<'cmd> {
    cmd: &'cmd mut Command,
    cur_idx: Cell<usize>,
    /// Index of the previous flag subcommand in a group of flags.
    flag_subcmd_at: Option<usize>,
    /// Counter indicating the number of items to skip
    /// when revisiting the group of flags which includes the flag subcommand.
    flag_subcmd_skip: usize,
}

// Initializing Methods
impl<'cmd> Parser<'cmd> {
    pub(crate) fn new(cmd: &'cmd mut Command) -> Self {
        Parser {
            cmd,
            cur_idx: Cell::new(0),
            flag_subcmd_at: None,
            flag_subcmd_skip: 0,
        }
    }
}

// Parsing Methods
impl<'cmd> Parser<'cmd> {
    // The actual parsing function
    #[allow(clippy::cognitive_complexity)]
    pub(crate) fn get_matches_with(
        &mut self,
        matcher: &mut ArgMatcher,
        raw_args: &mut clap_lex::RawArgs,
        args_cursor: clap_lex::ArgCursor,
    ) -> ClapResult<()> {
        debug!("Parser::get_matches_with");

        ok!(self.parse(matcher, raw_args, args_cursor).map_err(|err| {
            if self.cmd.is_ignore_errors_set() {
                #[cfg(feature = "env")]
                let _ = self.add_env(matcher);
                let _ = self.add_defaults(matcher);
            }
            err
        }));
        ok!(self.resolve_pending(matcher));

        #[cfg(feature = "env")]
        ok!(self.add_env(matcher));
        ok!(self.add_defaults(matcher));

        Validator::new(self.cmd).validate(matcher)
    }

    // The actual parsing function
    #[allow(clippy::cognitive_complexity)]
    pub(crate) fn parse(
        &mut self,
        matcher: &mut ArgMatcher,
        raw_args: &mut clap_lex::RawArgs,
        mut args_cursor: clap_lex::ArgCursor,
    ) -> ClapResult<()> {
        debug!("Parser::parse");
        // Verify all positional assertions pass

        let mut subcmd_name: Option<String> = None;
        let mut keep_state = false;
        let mut parse_state = ParseState::ValuesDone;
        let mut pos_counter = 1;

        // Already met any valid arg(then we shouldn't expect subcommands after it).
        let mut valid_arg_found = false;
        // If the user already passed '--'. Meaning only positional args follow.
        let mut trailing_values = false;

        // Count of positional args
        let positional_count = self
            .cmd
            .get_keymap()
            .keys()
            .filter(|x| x.is_position())
            .count();
        // If any arg sets .last(true)
        let contains_last = self.cmd.get_arguments().any(|x| x.is_last_set());

        while let Some(arg_os) = raw_args.next(&mut args_cursor) {
            debug!(
                "Parser::get_matches_with: Begin parsing '{:?}'",
                arg_os.to_value_os(),
            );

            // Has the user already passed '--'? Meaning only positional args follow
            if !trailing_values {
                if self.cmd.is_subcommand_precedence_over_arg_set()
                    || !matches!(parse_state, ParseState::Opt(_) | ParseState::Pos(_))
                {
                    // Does the arg match a subcommand name, or any of its aliases (if defined)
                    let sc_name = self.possible_subcommand(arg_os.to_value(), valid_arg_found);
                    debug!("Parser::get_matches_with: sc={sc_name:?}");
                    if let Some(sc_name) = sc_name {
                        if sc_name == "help" && !self.cmd.is_disable_help_subcommand_set() {
                            ok!(self.parse_help_subcommand(raw_args.remaining(&mut args_cursor)));
                            unreachable!("`parse_help_subcommand` always errors");
                        } else {
                            subcmd_name = Some(sc_name.to_owned());
                        }
                        break;
                    }
                }

                if arg_os.is_escape() {
                    if matches!(&parse_state, ParseState::Opt(opt) | ParseState::Pos(opt) if
                        self.cmd[opt].is_allow_hyphen_values_set())
                    {
                        // ParseResult::MaybeHyphenValue, do nothing
                    } else {
                        debug!("Parser::get_matches_with: setting TrailingVals=true");
                        trailing_values = true;
                        matcher.start_trailing();
                        continue;
                    }
                } else if let Some((long_arg, long_value)) = arg_os.to_long() {
                    let parse_result = ok!(self.parse_long_arg(
                        matcher,
                        long_arg,
                        long_value,
                        &parse_state,
                        pos_counter,
                        &mut valid_arg_found,
                    ));
                    debug!("Parser::get_matches_with: After parse_long_arg {parse_result:?}");
                    match parse_result {
                        ParseResult::NoArg => {
                            unreachable!("`to_long` always has the flag specified")
                        }
                        ParseResult::ValuesDone => {
                            parse_state = ParseState::ValuesDone;
                            continue;
                        }
                        ParseResult::Opt(id) => {
                            parse_state = ParseState::Opt(id);
                            continue;
                        }
                        ParseResult::FlagSubCommand(name) => {
                            debug!(
                                "Parser::get_matches_with: FlagSubCommand found in long arg {:?}",
                                &name
                            );
                            subcmd_name = Some(name);
                            break;
                        }
                        ParseResult::EqualsNotProvided { arg } => {
                            let _ = self.resolve_pending(matcher);
                            return Err(ClapError::no_equals(
                                self.cmd,
                                arg,
                                Usage::new(self.cmd).create_usage_with_title(&[]),
                            ));
                        }
                        ParseResult::NoMatchingArg { arg } => {
                            let _ = self.resolve_pending(matcher);
                            let remaining_args: Vec<_> =
                                raw_args.remaining(&mut args_cursor).collect();
                            return Err(self.did_you_mean_error(
                                &arg,
                                matcher,
                                &remaining_args,
                                trailing_values,
                            ));
                        }
                        ParseResult::UnneededAttachedValue { rest, used, arg } => {
                            let _ = self.resolve_pending(matcher);
                            return Err(ClapError::too_many_values(
                                self.cmd,
                                rest,
                                arg,
                                Usage::new(self.cmd).create_usage_with_title(&used),
                            ));
                        }
                        ParseResult::MaybeHyphenValue => {
                            // Maybe a hyphen value, do nothing.
                        }
                        ParseResult::AttachedValueNotConsumed => {
                            unreachable!()
                        }
                    }
                } else if let Some(short_arg) = arg_os.to_short() {
                    // Arg looks like a short flag, and not a possible number

                    // Try to parse short args like normal, if allow_hyphen_values or
                    // AllowNegativeNumbers is set, parse_short_arg will *not* throw
                    // an error, and instead return Ok(None)
                    let parse_result = ok!(self.parse_short_arg(
                        matcher,
                        short_arg,
                        &parse_state,
                        pos_counter,
                        &mut valid_arg_found,
                    ));
                    // If it's None, we then check if one of those two AppSettings was set
                    debug!("Parser::get_matches_with: After parse_short_arg {parse_result:?}");
                    match parse_result {
                        ParseResult::NoArg => {
                            // Is a single dash `-`, try positional.
                        }
                        ParseResult::ValuesDone => {
                            parse_state = ParseState::ValuesDone;
                            continue;
                        }
                        ParseResult::Opt(id) => {
                            parse_state = ParseState::Opt(id);
                            continue;
                        }
                        ParseResult::FlagSubCommand(name) => {
                            // If there are more short flags to be processed, we should keep the state, and later
                            // revisit the current group of short flags skipping the subcommand.
                            keep_state = self
                                .flag_subcmd_at
                                .map(|at| {
                                    raw_args
                                        .seek(&mut args_cursor, clap_lex::SeekFrom::Current(-1));
                                    // Since we are now saving the current state, the number of flags to skip during state recovery should
                                    // be the current index (`cur_idx`) minus ONE UNIT TO THE LEFT of the starting position.
                                    self.flag_subcmd_skip = self.cur_idx.get() - at + 1;
                                })
                                .is_some();

                            debug!(
                                "Parser::get_matches_with:FlagSubCommandShort: subcmd_name={}, keep_state={}, flag_subcmd_skip={}",
                                name,
                                keep_state,
                                self.flag_subcmd_skip
                            );

                            subcmd_name = Some(name);
                            break;
                        }
                        ParseResult::EqualsNotProvided { arg } => {
                            let _ = self.resolve_pending(matcher);
                            return Err(ClapError::no_equals(
                                self.cmd,
                                arg,
                                Usage::new(self.cmd).create_usage_with_title(&[]),
                            ));
                        }
                        ParseResult::NoMatchingArg { arg } => {
                            let _ = self.resolve_pending(matcher);
                            // We already know it looks like a flag
                            let suggested_trailing_arg =
                                !trailing_values && self.cmd.has_positionals();
                            return Err(ClapError::unknown_argument(
                                self.cmd,
                                arg,
                                None,
                                suggested_trailing_arg,
                                Usage::new(self.cmd).create_usage_with_title(&[]),
                            ));
                        }
                        ParseResult::MaybeHyphenValue => {
                            // Maybe a hyphen value, do nothing.
                        }
                        ParseResult::UnneededAttachedValue { .. }
                        | ParseResult::AttachedValueNotConsumed => unreachable!(),
                    }
                }

                if let ParseState::Opt(id) = &parse_state {
                    // Assume this is a value of a previous arg.

                    // get the option so we can check the settings
                    let arg = &self.cmd[id];
                    let parse_result = if let Some(parse_result) =
                        self.check_terminator(arg, arg_os.to_value_os())
                    {
                        parse_result
                    } else {
                        let trailing_values = false;
                        let arg_values = matcher.pending_values_mut(id, None, trailing_values);
                        arg_values.push(arg_os.to_value_os().to_owned());
                        if matcher.needs_more_vals(arg) {
                            ParseResult::Opt(arg.get_id().clone())
                        } else {
                            ParseResult::ValuesDone
                        }
                    };
                    parse_state = match parse_result {
                        ParseResult::Opt(id) => ParseState::Opt(id),
                        ParseResult::ValuesDone => ParseState::ValuesDone,
                        _ => unreachable!(),
                    };
                    // get the next value from the iterator
                    continue;
                }
            }

            // Correct pos_counter.
            pos_counter = {
                let is_second_to_last = pos_counter + 1 == positional_count;

                // The last positional argument, or second to last positional
                // argument may be set to .multiple_values(true) or `.multiple_occurrences(true)`
                let low_index_mults = is_second_to_last
                    && self.cmd.get_positionals().any(|a| {
                        a.is_multiple() && (positional_count != a.get_index().unwrap_or(0))
                    })
                    && self
                        .cmd
                        .get_positionals()
                        .last()
                        .map(|p_name| !p_name.is_last_set())
                        .unwrap_or_default();

                let is_terminated = self
                    .cmd
                    .get_keymap()
                    .get(&pos_counter)
                    .map(|a| a.get_value_terminator().is_some())
                    .unwrap_or_default();

                let missing_pos = self.cmd.is_allow_missing_positional_set()
                    && is_second_to_last
                    && !trailing_values;

                debug!("Parser::get_matches_with: Positional counter...{pos_counter}");
                debug!("Parser::get_matches_with: Low index multiples...{low_index_mults:?}");

                if (low_index_mults || missing_pos) && !is_terminated {
                    let skip_current = if let Some(n) = raw_args.peek(&args_cursor) {
                        if let Some(arg) = self
                            .cmd
                            .get_positionals()
                            .find(|a| a.get_index() == Some(pos_counter))
                        {
                            // If next value looks like a new_arg or it's a
                            // subcommand, skip positional argument under current
                            // pos_counter(which means current value cannot be a
                            // positional argument with a value next to it), assume
                            // current value matches the next arg.
                            self.is_new_arg(&n, arg)
                                || self
                                    .possible_subcommand(n.to_value(), valid_arg_found)
                                    .is_some()
                        } else {
                            true
                        }
                    } else {
                        true
                    };

                    if skip_current {
                        debug!("Parser::get_matches_with: Bumping the positional counter...");
                        pos_counter + 1
                    } else {
                        pos_counter
                    }
                } else if trailing_values
                    && (self.cmd.is_allow_missing_positional_set() || contains_last)
                {
                    // Came to -- and one positional has .last(true) set, so we go immediately
                    // to the last (highest index) positional
                    debug!("Parser::get_matches_with: .last(true) and --, setting last pos");
                    positional_count
                } else {
                    pos_counter
                }
            };

            if let Some(arg) = self.cmd.get_keymap().get(&pos_counter) {
                if arg.is_last_set() && !trailing_values {
                    let _ = self.resolve_pending(matcher);
                    // Its already considered a positional, we don't need to suggest turning it
                    // into one
                    let suggested_trailing_arg = false;
                    return Err(ClapError::unknown_argument(
                        self.cmd,
                        arg_os.display().to_string(),
                        None,
                        suggested_trailing_arg,
                        Usage::new(self.cmd).create_usage_with_title(&[]),
                    ));
                }

                if arg.is_trailing_var_arg_set() {
                    trailing_values = true;
                }

                if matcher.pending_arg_id() != Some(arg.get_id()) || !arg.is_multiple_values_set() {
                    ok!(self.resolve_pending(matcher));
                }
                parse_state =
                    if let Some(parse_result) = self.check_terminator(arg, arg_os.to_value_os()) {
                        debug_assert_eq!(parse_result, ParseResult::ValuesDone);
                        pos_counter += 1;
                        ParseState::ValuesDone
                    } else {
                        let arg_values = matcher.pending_values_mut(
                            arg.get_id(),
                            Some(Identifier::Index),
                            trailing_values,
                        );
                        arg_values.push(arg_os.to_value_os().to_owned());

                        // Only increment the positional counter if it doesn't allow multiples
                        if !arg.is_multiple() {
                            pos_counter += 1;
                            ParseState::ValuesDone
                        } else {
                            ParseState::Pos(arg.get_id().clone())
                        }
                    };
                valid_arg_found = true;
            } else if let Some(external_parser) =
                self.cmd.get_external_subcommand_value_parser().cloned()
            {
                // Get external subcommand name
                let sc_name = match arg_os.to_value() {
                    Ok(s) => s.to_owned(),
                    Err(_) => {
                        let _ = self.resolve_pending(matcher);
                        return Err(ClapError::invalid_utf8(
                            self.cmd,
                            Usage::new(self.cmd).create_usage_with_title(&[]),
                        ));
                    }
                };

                // Collect the external subcommand args
                let mut sc_m = ArgMatcher::new(self.cmd);
                sc_m.start_occurrence_of_external(self.cmd);

                for raw_val in raw_args.remaining(&mut args_cursor) {
                    let val = ok!(external_parser.parse_ref(
                        self.cmd,
                        None,
                        raw_val,
                        ValueSource::CommandLine
                    ));
                    let external_id = Id::from_static_ref(Id::EXTERNAL);
                    sc_m.add_val_to(&external_id, val, raw_val.to_os_string());
                }

                matcher.subcommand(SubCommand {
                    name: sc_name,
                    matches: sc_m.into_inner(),
                });

                return Ok(());
            } else {
                // Start error processing
                let _ = self.resolve_pending(matcher);
                return Err(self.match_arg_error(
                    &arg_os,
                    valid_arg_found,
                    trailing_values,
                    matcher,
                ));
            }
        }

        if let Some(ref pos_sc_name) = subcmd_name {
            if self.cmd.is_args_conflicts_with_subcommands_set() && valid_arg_found {
                return Err(ClapError::subcommand_conflict(
                    self.cmd,
                    pos_sc_name.clone(),
                    matcher
                        .arg_ids()
                        .map(|id| self.cmd.find(id).unwrap().to_string())
                        .collect(),
                    Usage::new(self.cmd).create_usage_with_title(&[]),
                ));
            }
            let sc_name = self
                .cmd
                .find_subcommand(pos_sc_name)
                .expect(INTERNAL_ERROR_MSG)
                .get_name()
                .to_owned();
            ok!(self.parse_subcommand(&sc_name, matcher, raw_args, args_cursor, keep_state));
        }

        Ok(())
    }

    fn match_arg_error(
        &self,
        arg_os: &clap_lex::ParsedArg<'_>,
        valid_arg_found: bool,
        trailing_values: bool,
        matcher: &ArgMatcher,
    ) -> ClapError {
        // If argument follows a `--`
        if trailing_values {
            // If the arg matches a subcommand name, or any of its aliases (if defined)
            if self
                .possible_subcommand(arg_os.to_value(), valid_arg_found)
                .is_some()
            {
                return ClapError::unnecessary_double_dash(
                    self.cmd,
                    arg_os.display().to_string(),
                    Usage::new(self.cmd).create_usage_with_title(&[]),
                );
            }
        }

        let suggested_trailing_arg = !trailing_values
            && self.cmd.has_positionals()
            && (arg_os.is_long() || arg_os.is_short());

        if self.cmd.has_subcommands() {
            if self.cmd.is_args_conflicts_with_subcommands_set() && valid_arg_found {
                return ClapError::subcommand_conflict(
                    self.cmd,
                    arg_os.display().to_string(),
                    matcher
                        .arg_ids()
                        .filter_map(|id| self.cmd.find(id).map(|a| a.to_string()))
                        .collect(),
                    Usage::new(self.cmd).create_usage_with_title(&[]),
                );
            }

            let candidates = suggestions::did_you_mean(
                &arg_os.display().to_string(),
                self.cmd.all_subcommand_names(),
            );
            // If the argument looks like a subcommand.
            if !candidates.is_empty() {
                return ClapError::invalid_subcommand(
                    self.cmd,
                    arg_os.display().to_string(),
                    candidates,
                    self.cmd.get_bin_name_fallback().to_owned(),
                    suggested_trailing_arg,
                    Usage::new(self.cmd).create_usage_with_title(&[]),
                );
            }

            // If the argument must be a subcommand.
            if !self.cmd.has_positionals() || self.cmd.is_infer_subcommands_set() {
                return ClapError::unrecognized_subcommand(
                    self.cmd,
                    arg_os.display().to_string(),
                    Usage::new(self.cmd).create_usage_with_title(&[]),
                );
            }
        }

        ClapError::unknown_argument(
            self.cmd,
            arg_os.display().to_string(),
            None,
            suggested_trailing_arg,
            Usage::new(self.cmd).create_usage_with_title(&[]),
        )
    }

    // Checks if the arg matches a subcommand name, or any of its aliases (if defined)
    fn possible_subcommand(
        &self,
        arg: Result<&str, &OsStr>,
        valid_arg_found: bool,
    ) -> Option<&str> {
        debug!("Parser::possible_subcommand: arg={arg:?}");
        let arg = some!(arg.ok());

        if !(self.cmd.is_args_conflicts_with_subcommands_set() && valid_arg_found) {
            if self.cmd.is_infer_subcommands_set() {
                // For subcommand `test`, we accepts it's prefix: `t`, `te`,
                // `tes` and `test`.
                let mut iter = self.cmd.get_subcommands().filter_map(|s| {
                    if s.get_name().starts_with(arg) {
                        return Some(s.get_name());
                    }

                    // Use find here instead of chaining the iterator because we want to accept
                    // conflicts in aliases.
                    s.get_all_aliases().find(|s| s.starts_with(arg))
                });

                if let name @ Some(_) = iter.next() {
                    if iter.next().is_none() {
                        return name;
                    }
                }
            }
            // Don't use an else here because we want inference to support exact matching even if
            // there are conflicts.
            if let Some(sc) = self.cmd.find_subcommand(arg) {
                return Some(sc.get_name());
            }
        }
        None
    }

    // Checks if the arg matches a long flag subcommand name, or any of its aliases (if defined)
    fn possible_long_flag_subcommand(&self, arg: &str) -> Option<&str> {
        debug!("Parser::possible_long_flag_subcommand: arg={arg:?}");
        if self.cmd.is_infer_subcommands_set() {
            let mut iter = self.cmd.get_subcommands().filter_map(|sc| {
                sc.get_long_flag().and_then(|long| {
                    if long.starts_with(arg) {
                        Some(sc.get_name())
                    } else {
                        sc.get_all_long_flag_aliases().find_map(|alias| {
                            if alias.starts_with(arg) {
                                Some(sc.get_name())
                            } else {
                                None
                            }
                        })
                    }
                })
            });

            if let name @ Some(_) = iter.next() {
                if iter.next().is_none() {
                    return name;
                }
            }
        }
        if let Some(sc_name) = self.cmd.find_long_subcmd(arg) {
            return Some(sc_name);
        }
        None
    }

    fn parse_help_subcommand(
        &self,
        cmds: impl Iterator<Item = &'cmd OsStr>,
    ) -> ClapResult<std::convert::Infallible> {
        debug!("Parser::parse_help_subcommand");

        let mut cmd = self.cmd.clone();
        let sc = {
            let mut sc = &mut cmd;

            for cmd in cmds {
                sc = if let Some(sc_name) =
                    sc.find_subcommand(cmd).map(|sc| sc.get_name().to_owned())
                {
                    sc._build_subcommand(&sc_name).unwrap()
                } else {
                    return Err(ClapError::unrecognized_subcommand(
                        sc,
                        cmd.to_string_lossy().into_owned(),
                        Usage::new(sc).create_usage_with_title(&[]),
                    ));
                };
            }

            sc
        };
        let parser = Parser::new(sc);

        Err(parser.help_err(true))
    }

    fn is_new_arg(&self, next: &clap_lex::ParsedArg<'_>, current_positional: &Arg) -> bool {
        #![allow(clippy::needless_bool)] // Prefer consistent if/else-if ladder

        debug!(
            "Parser::is_new_arg: {:?}:{}",
            next.to_value_os(),
            current_positional.get_id()
        );

        if self.cmd[current_positional.get_id()].is_allow_hyphen_values_set()
            || (self.cmd[current_positional.get_id()].is_allow_negative_numbers_set()
                && next.is_negative_number())
        {
            // If allow hyphen, this isn't a new arg.
            debug!("Parser::is_new_arg: Allow hyphen");
            false
        } else if next.is_long() {
            // If this is a long flag, this is a new arg.
            debug!("Parser::is_new_arg: --<something> found");
            true
        } else if next.is_short() {
            // If this is a short flag, this is a new arg. But a single '-' by
            // itself is a value and typically means "stdin" on unix systems.
            debug!("Parser::is_new_arg: -<something> found");
            true
        } else {
            // Nothing special, this is a value.
            debug!("Parser::is_new_arg: value");
            false
        }
    }

    fn parse_subcommand(
        &mut self,
        sc_name: &str,
        matcher: &mut ArgMatcher,
        raw_args: &mut clap_lex::RawArgs,
        args_cursor: clap_lex::ArgCursor,
        keep_state: bool,
    ) -> ClapResult<()> {
        debug!("Parser::parse_subcommand");

        let partial_parsing_enabled = self.cmd.is_ignore_errors_set();

        if let Some(sc) = self.cmd._build_subcommand(sc_name) {
            let mut sc_matcher = ArgMatcher::new(sc);

            debug!(
                "Parser::parse_subcommand: About to parse sc={}",
                sc.get_name()
            );

            {
                let mut p = Parser::new(sc);
                // HACK: maintain indexes between parsers
                // FlagSubCommand short arg needs to revisit the current short args, but skip the subcommand itself
                if keep_state {
                    p.cur_idx.set(self.cur_idx.get());
                    p.flag_subcmd_at = self.flag_subcmd_at;
                    p.flag_subcmd_skip = self.flag_subcmd_skip;
                }
                if let Err(error) = p.get_matches_with(&mut sc_matcher, raw_args, args_cursor) {
                    if partial_parsing_enabled {
                        debug!("Parser::parse_subcommand: ignored error in subcommand {sc_name}: {error:?}");
                    } else {
                        return Err(error);
                    }
                }
            }
            matcher.subcommand(SubCommand {
                name: sc.get_name().to_owned(),
                matches: sc_matcher.into_inner(),
            });
        }
        Ok(())
    }

    fn parse_long_arg(
        &mut self,
        matcher: &mut ArgMatcher,
        long_arg: Result<&str, &OsStr>,
        long_value: Option<&OsStr>,
        parse_state: &ParseState,
        pos_counter: usize,
        valid_arg_found: &mut bool,
    ) -> ClapResult<ParseResult> {
        // maybe here lifetime should be 'a
        debug!("Parser::parse_long_arg");

        #[allow(clippy::blocks_in_conditions)]
        if matches!(parse_state, ParseState::Opt(opt) | ParseState::Pos(opt) if
            self.cmd[opt].is_allow_hyphen_values_set())
        {
            debug!("Parser::parse_long_arg: prior arg accepts hyphenated values",);
            return Ok(ParseResult::MaybeHyphenValue);
        }

        debug!("Parser::parse_long_arg: Does it contain '='...");
        let long_arg = match long_arg {
            Ok(long_arg) => long_arg,
            Err(long_arg_os) => {
                return Ok(ParseResult::NoMatchingArg {
                    arg: long_arg_os.to_string_lossy().into_owned(),
                })
            }
        };
        if long_arg.is_empty() {
            debug_assert!(
                long_value.is_some(),
                "`--` should be filtered out before this point"
            );
        }

        let arg = if let Some(arg) = self.cmd.get_keymap().get(long_arg) {
            debug!("Parser::parse_long_arg: Found valid arg or flag '{arg}'");
            Some((long_arg, arg))
        } else if self.cmd.is_infer_long_args_set() {
            let mut iter = self.cmd.get_arguments().filter_map(|a| {
                if let Some(long) = a.get_long() {
                    if long.starts_with(long_arg) {
                        return Some((long, a));
                    }
                }
                a.aliases
                    .iter()
                    .find_map(|(alias, _)| alias.starts_with(long_arg).then(|| (alias.as_str(), a)))
            });

            iter.next().filter(|_| iter.next().is_none())
        } else {
            None
        };

        if let Some((_long_arg, arg)) = arg {
            let ident = Identifier::Long;
            *valid_arg_found = true;
            if arg.is_takes_value_set() {
                debug!(
                    "Parser::parse_long_arg({:?}): Found an arg with value '{:?}'",
                    long_arg, &long_value
                );
                let has_eq = long_value.is_some();
                self.parse_opt_value(ident, long_value, arg, matcher, has_eq)
            } else if let Some(rest) = long_value {
                let required = self.cmd.required_graph();
                debug!("Parser::parse_long_arg({long_arg:?}): Got invalid literal `{rest:?}`");
                let mut used: Vec<Id> = matcher
                    .arg_ids()
                    .filter(|arg_id| {
                        matcher.check_explicit(arg_id, &crate::builder::ArgPredicate::IsPresent)
                    })
                    .filter(|&n| {
                        self.cmd
                            .find(n)
                            .map(|a| !(a.is_hide_set() || required.contains(a.get_id())))
                            .unwrap_or(true)
                    })
                    .cloned()
                    .collect();
                used.push(arg.get_id().clone());

                Ok(ParseResult::UnneededAttachedValue {
                    rest: rest.to_string_lossy().into_owned(),
                    used,
                    arg: arg.to_string(),
                })
            } else {
                debug!("Parser::parse_long_arg({long_arg:?}): Presence validated");
                let trailing_idx = None;
                self.react(
                    Some(ident),
                    ValueSource::CommandLine,
                    arg,
                    vec![],
                    trailing_idx,
                    matcher,
                )
            }
        } else if let Some(sc_name) = self.possible_long_flag_subcommand(long_arg) {
            Ok(ParseResult::FlagSubCommand(sc_name.to_string()))
        } else if self
            .cmd
            .get_keymap()
            .get(&pos_counter)
            .map(|arg| arg.is_allow_hyphen_values_set() && !arg.is_last_set())
            .unwrap_or_default()
        {
            debug!("Parser::parse_long_args: positional at {pos_counter} allows hyphens");
            Ok(ParseResult::MaybeHyphenValue)
        } else {
            Ok(ParseResult::NoMatchingArg {
                arg: long_arg.to_owned(),
            })
        }
    }

    fn parse_short_arg(
        &mut self,
        matcher: &mut ArgMatcher,
        mut short_arg: clap_lex::ShortFlags<'_>,
        parse_state: &ParseState,
        // change this to possible pos_arg when removing the usage of &mut Parser.
        pos_counter: usize,
        valid_arg_found: &mut bool,
    ) -> ClapResult<ParseResult> {
        debug!("Parser::parse_short_arg: short_arg={short_arg:?}");

        #[allow(clippy::blocks_in_conditions)]
        if matches!(parse_state, ParseState::Opt(opt) | ParseState::Pos(opt)
                if self.cmd[opt].is_allow_hyphen_values_set() || (self.cmd[opt].is_allow_negative_numbers_set() && short_arg.is_negative_number()))
        {
            debug!("Parser::parse_short_args: prior arg accepts hyphenated values",);
            return Ok(ParseResult::MaybeHyphenValue);
        } else if self
            .cmd
            .get_keymap()
            .get(&pos_counter)
            .map(|arg| arg.is_allow_negative_numbers_set())
            .unwrap_or_default()
            && short_arg.is_negative_number()
        {
            debug!("Parser::parse_short_arg: negative number");
            return Ok(ParseResult::MaybeHyphenValue);
        } else if self
            .cmd
            .get_keymap()
            .get(&pos_counter)
            .map(|arg| arg.is_allow_hyphen_values_set() && !arg.is_last_set())
            .unwrap_or_default()
            && short_arg
                .clone()
                .any(|c| !c.map(|c| self.cmd.contains_short(c)).unwrap_or_default())
        {
            debug!("Parser::parse_short_args: positional at {pos_counter} allows hyphens");
            return Ok(ParseResult::MaybeHyphenValue);
        }

        let mut ret = ParseResult::NoArg;

        let skip = self.flag_subcmd_skip;
        self.flag_subcmd_skip = 0;
        let res = short_arg.advance_by(skip);
        debug_assert_eq!(
            res,
            Ok(()),
            "tracking of `flag_subcmd_skip` is off for `{short_arg:?}`"
        );
        while let Some(c) = short_arg.next_flag() {
            let c = match c {
                Ok(c) => c,
                Err(rest) => {
                    return Ok(ParseResult::NoMatchingArg {
                        arg: format!("-{}", rest.to_string_lossy()),
                    });
                }
            };
            debug!("Parser::parse_short_arg:iter:{c}");

            // Check for matching short options, and return the name if there is no trailing
            // concatenated value: -oval
            // Option: -o
            // Value: val
            if let Some(arg) = self.cmd.get_keymap().get(&c) {
                let ident = Identifier::Short;
                debug!("Parser::parse_short_arg:iter:{c}: Found valid opt or flag");
                *valid_arg_found = true;
                if !arg.is_takes_value_set() {
                    let arg_values = Vec::new();
                    let trailing_idx = None;
                    ret = ok!(self.react(
                        Some(ident),
                        ValueSource::CommandLine,
                        arg,
                        arg_values,
                        trailing_idx,
                        matcher,
                    ));
                    continue;
                }

                // Check for trailing concatenated value
                //
                // Cloning the iterator, so we rollback if it isn't there.
                let val = short_arg.clone().next_value_os().unwrap_or_default();
                debug!("Parser::parse_short_arg:iter:{c}: val={val:?}, short_arg={short_arg:?}");
                let val = Some(val).filter(|v| !v.is_empty());

                // Default to "we're expecting a value later".
                //
                // If attached value is not consumed, we may have more short
                // flags to parse, continue.
                //
                // e.g. `-xvf`, when require_equals && x.min_vals == 0, we don't
                // consume the `vf`, even if it's provided as value.
                let (val, has_eq) = if let Some(val) = val.and_then(|v| v.strip_prefix("=")) {
                    (Some(val), true)
                } else {
                    (val, false)
                };
                match ok!(self.parse_opt_value(ident, val, arg, matcher, has_eq)) {
                    ParseResult::AttachedValueNotConsumed => continue,
                    x => return Ok(x),
                }
            }

            return if let Some(sc_name) = self.cmd.find_short_subcmd(c) {
                debug!("Parser::parse_short_arg:iter:{c}: subcommand={sc_name}");
                // Make sure indices get updated before reading `self.cur_idx`
                ok!(self.resolve_pending(matcher));
                self.cur_idx.set(self.cur_idx.get() + 1);
                debug!("Parser::parse_short_arg: cur_idx:={}", self.cur_idx.get());

                let name = sc_name.to_string();
                // Get the index of the previously saved flag subcommand in the group of flags (if exists).
                // If it is a new flag subcommand, then the formentioned index should be the current one
                // (ie. `cur_idx`), and should be registered.
                let cur_idx = self.cur_idx.get();
                self.flag_subcmd_at.get_or_insert(cur_idx);
                let done_short_args = short_arg.is_empty();
                if done_short_args {
                    self.flag_subcmd_at = None;
                }
                Ok(ParseResult::FlagSubCommand(name))
            } else {
                Ok(ParseResult::NoMatchingArg {
                    arg: format!("-{c}"),
                })
            };
        }
        Ok(ret)
    }

    fn parse_opt_value(
        &self,
        ident: Identifier,
        attached_value: Option<&OsStr>,
        arg: &Arg,
        matcher: &mut ArgMatcher,
        has_eq: bool,
    ) -> ClapResult<ParseResult> {
        debug!(
            "Parser::parse_opt_value; arg={}, val={:?}, has_eq={:?}",
            arg.get_id(),
            attached_value,
            has_eq
        );
        debug!("Parser::parse_opt_value; arg.settings={:?}", arg.settings);

        debug!("Parser::parse_opt_value; Checking for val...");
        // require_equals is set, but no '=' is provided, try throwing error.
        if arg.is_require_equals_set() && !has_eq {
            if arg.get_min_vals() == 0 {
                debug!("Requires equals, but min_vals == 0");
                let arg_values = Vec::new();
                let trailing_idx = None;
                let react_result = ok!(self.react(
                    Some(ident),
                    ValueSource::CommandLine,
                    arg,
                    arg_values,
                    trailing_idx,
                    matcher,
                ));
                debug_assert_eq!(react_result, ParseResult::ValuesDone);
                if attached_value.is_some() {
                    Ok(ParseResult::AttachedValueNotConsumed)
                } else {
                    Ok(ParseResult::ValuesDone)
                }
            } else {
                debug!("Requires equals but not provided. Error.");
                Ok(ParseResult::EqualsNotProvided {
                    arg: arg.to_string(),
                })
            }
        } else if let Some(v) = attached_value {
            let arg_values = vec![v.to_owned()];
            let trailing_idx = None;
            let react_result = ok!(self.react(
                Some(ident),
                ValueSource::CommandLine,
                arg,
                arg_values,
                trailing_idx,
                matcher,
            ));
            debug_assert_eq!(react_result, ParseResult::ValuesDone);
            // Attached are always done
            Ok(ParseResult::ValuesDone)
        } else {
            debug!("Parser::parse_opt_value: More arg vals required...");
            ok!(self.resolve_pending(matcher));
            let trailing_values = false;
            matcher.pending_values_mut(arg.get_id(), Some(ident), trailing_values);
            Ok(ParseResult::Opt(arg.get_id().clone()))
        }
    }

    fn check_terminator(&self, arg: &Arg, val: &OsStr) -> Option<ParseResult> {
        if Some(val) == arg.terminator.as_ref().map(|s| OsStr::new(s.as_str())) {
            debug!("Parser::check_terminator: terminator={:?}", arg.terminator);
            Some(ParseResult::ValuesDone)
        } else {
            None
        }
    }

    fn push_arg_values(
        &self,
        arg: &Arg,
        raw_vals: Vec<OsString>,
        source: ValueSource,
        matcher: &mut ArgMatcher,
    ) -> ClapResult<()> {
        debug!("Parser::push_arg_values: {raw_vals:?}");

        for raw_val in raw_vals {
            // update the current index because each value is a distinct index to clap
            self.cur_idx.set(self.cur_idx.get() + 1);
            debug!(
                "Parser::add_single_val_to_arg: cur_idx:={}",
                self.cur_idx.get()
            );
            let value_parser = arg.get_value_parser();
            let val = ok!(value_parser.parse_ref(self.cmd, Some(arg), &raw_val, source));

            matcher.add_val_to(arg.get_id(), val, raw_val);
            matcher.add_index_to(arg.get_id(), self.cur_idx.get());
        }

        Ok(())
    }

    fn resolve_pending(&self, matcher: &mut ArgMatcher) -> ClapResult<()> {
        let pending = match matcher.take_pending() {
            Some(pending) => pending,
            None => {
                return Ok(());
            }
        };

        debug!("Parser::resolve_pending: id={:?}", pending.id);
        let arg = self.cmd.find(&pending.id).expect(INTERNAL_ERROR_MSG);
        let _ = ok!(self.react(
            pending.ident,
            ValueSource::CommandLine,
            arg,
            pending.raw_vals,
            pending.trailing_idx,
            matcher,
        ));

        Ok(())
    }

    fn react(
        &self,
        ident: Option<Identifier>,
        source: ValueSource,
        arg: &Arg,
        mut raw_vals: Vec<OsString>,
        mut trailing_idx: Option<usize>,
        matcher: &mut ArgMatcher,
    ) -> ClapResult<ParseResult> {
        ok!(self.resolve_pending(matcher));

        debug!(
            "Parser::react action={:?}, identifier={:?}, source={:?}",
            arg.get_action(),
            ident,
            source
        );

        // Process before `default_missing_values` to avoid it counting as values from the command
        // line
        if source == ValueSource::CommandLine {
            ok!(self.verify_num_args(arg, &raw_vals));
        }

        if raw_vals.is_empty() {
            // We assume this case is valid: require equals, but min_vals == 0.
            if !arg.default_missing_vals.is_empty() {
                debug!("Parser::react: has default_missing_vals");
                trailing_idx = None;
                raw_vals.extend(
                    arg.default_missing_vals
                        .iter()
                        .map(|s| s.as_os_str().to_owned()),
                );
            }
        }

        if let Some(val_delim) = arg.get_value_delimiter() {
            if self.cmd.is_dont_delimit_trailing_values_set() && trailing_idx == Some(0) {
                // Nothing to do
            } else {
                let mut val_delim_buffer = [0; 4];
                let val_delim = val_delim.encode_utf8(&mut val_delim_buffer);
                let mut split_raw_vals = Vec::with_capacity(raw_vals.len());
                for (i, raw_val) in raw_vals.into_iter().enumerate() {
                    if !raw_val.contains(val_delim)
                        || (self.cmd.is_dont_delimit_trailing_values_set()
                            && trailing_idx == Some(i))
                    {
                        split_raw_vals.push(raw_val);
                    } else {
                        split_raw_vals.extend(raw_val.split(val_delim).map(|x| x.to_owned()));
                    }
                }
                raw_vals = split_raw_vals;
            }
        }

        match arg.get_action() {
            ArgAction::Set => {
                if source == ValueSource::CommandLine
                    && matches!(ident, Some(Identifier::Short) | Some(Identifier::Long))
                {
                    // Record flag's index
                    self.cur_idx.set(self.cur_idx.get() + 1);
                    debug!("Parser::react: cur_idx:={}", self.cur_idx.get());
                }
                if matcher.remove(arg.get_id())
                    && !(self.cmd.is_args_override_self() || arg.overrides.contains(arg.get_id()))
                {
                    return Err(ClapError::argument_conflict(
                        self.cmd,
                        arg.to_string(),
                        vec![arg.to_string()],
                        Usage::new(self.cmd).create_usage_with_title(&[]),
                    ));
                }
                self.start_custom_arg(matcher, arg, source);
                ok!(self.push_arg_values(arg, raw_vals, source, matcher));
                if cfg!(debug_assertions) && matcher.needs_more_vals(arg) {
                    debug!(
                        "Parser::react not enough values passed in, leaving it to the validator to complain",
                    );
                }
                Ok(ParseResult::ValuesDone)
            }
            ArgAction::Append => {
                if source == ValueSource::CommandLine
                    && matches!(ident, Some(Identifier::Short) | Some(Identifier::Long))
                {
                    // Record flag's index
                    self.cur_idx.set(self.cur_idx.get() + 1);
                    debug!("Parser::react: cur_idx:={}", self.cur_idx.get());
                }
                self.start_custom_arg(matcher, arg, source);
                ok!(self.push_arg_values(arg, raw_vals, source, matcher));
                if cfg!(debug_assertions) && matcher.needs_more_vals(arg) {
                    debug!(
                        "Parser::react not enough values passed in, leaving it to the validator to complain",
                    );
                }
                Ok(ParseResult::ValuesDone)
            }
            ArgAction::SetTrue => {
                let raw_vals = if raw_vals.is_empty() {
                    vec![OsString::from("true")]
                } else {
                    raw_vals
                };

                if matcher.remove(arg.get_id())
                    && !(self.cmd.is_args_override_self() || arg.overrides.contains(arg.get_id()))
                {
                    return Err(ClapError::argument_conflict(
                        self.cmd,
                        arg.to_string(),
                        vec![arg.to_string()],
                        Usage::new(self.cmd).create_usage_with_title(&[]),
                    ));
                }
                self.start_custom_arg(matcher, arg, source);
                ok!(self.push_arg_values(arg, raw_vals, source, matcher));
                Ok(ParseResult::ValuesDone)
            }
            ArgAction::SetFalse => {
                let raw_vals = if raw_vals.is_empty() {
                    vec![OsString::from("false")]
                } else {
                    raw_vals
                };

                if matcher.remove(arg.get_id())
                    && !(self.cmd.is_args_override_self() || arg.overrides.contains(arg.get_id()))
                {
                    return Err(ClapError::argument_conflict(
                        self.cmd,
                        arg.to_string(),
                        vec![arg.to_string()],
                        Usage::new(self.cmd).create_usage_with_title(&[]),
                    ));
                }
                self.start_custom_arg(matcher, arg, source);
                ok!(self.push_arg_values(arg, raw_vals, source, matcher));
                Ok(ParseResult::ValuesDone)
            }
            ArgAction::Count => {
                let raw_vals = if raw_vals.is_empty() {
                    let existing_value = *matcher
                        .get_one::<crate::builder::CountType>(arg.get_id().as_str())
                        .unwrap_or(&0);
                    let next_value = existing_value.saturating_add(1);
                    vec![OsString::from(next_value.to_string())]
                } else {
                    raw_vals
                };

                matcher.remove(arg.get_id());
                self.start_custom_arg(matcher, arg, source);
                ok!(self.push_arg_values(arg, raw_vals, source, matcher));
                Ok(ParseResult::ValuesDone)
            }
            ArgAction::Help => {
                let use_long = match ident {
                    Some(Identifier::Long) => true,
                    Some(Identifier::Short) => false,
                    Some(Identifier::Index) => true,
                    None => true,
                };
                debug!("Help: use_long={use_long}");
                Err(self.help_err(use_long))
            }
            ArgAction::HelpShort => {
                let use_long = false;
                debug!("Help: use_long={use_long}");
                Err(self.help_err(use_long))
            }
            ArgAction::HelpLong => {
                let use_long = true;
                debug!("Help: use_long={use_long}");
                Err(self.help_err(use_long))
            }
            ArgAction::Version => {
                let use_long = match ident {
                    Some(Identifier::Long) => true,
                    Some(Identifier::Short) => false,
                    Some(Identifier::Index) => true,
                    None => true,
                };
                debug!("Version: use_long={use_long}");
                Err(self.version_err(use_long))
            }
        }
    }

    fn verify_num_args(&self, arg: &Arg, raw_vals: &[OsString]) -> ClapResult<()> {
        if self.cmd.is_ignore_errors_set() {
            return Ok(());
        }

        let actual = raw_vals.len();
        let expected = arg.get_num_args().expect(INTERNAL_ERROR_MSG);

        if 0 < expected.min_values() && actual == 0 {
            // Issue 665 (https://github.com/clap-rs/clap/issues/665)
            // Issue 1105 (https://github.com/clap-rs/clap/issues/1105)
            return Err(ClapError::empty_value(
                self.cmd,
                &super::get_possible_values_cli(arg)
                    .iter()
                    .filter(|pv| !pv.is_hide_set())
                    .map(|n| n.get_name().to_owned())
                    .collect::<Vec<_>>(),
                arg.to_string(),
            ));
        } else if let Some(expected) = expected.num_values() {
            if expected != actual {
                debug!("Validator::validate_arg_num_vals: Sending error WrongNumberOfValues");
                return Err(ClapError::wrong_number_of_values(
                    self.cmd,
                    arg.to_string(),
                    expected,
                    actual,
                    Usage::new(self.cmd).create_usage_with_title(&[]),
                ));
            }
        } else if actual < expected.min_values() {
            return Err(ClapError::too_few_values(
                self.cmd,
                arg.to_string(),
                expected.min_values(),
                actual,
                Usage::new(self.cmd).create_usage_with_title(&[]),
            ));
        } else if expected.max_values() < actual {
            debug!("Validator::validate_arg_num_vals: Sending error TooManyValues");
            return Err(ClapError::too_many_values(
                self.cmd,
                raw_vals
                    .last()
                    .expect(INTERNAL_ERROR_MSG)
                    .to_string_lossy()
                    .into_owned(),
                arg.to_string(),
                Usage::new(self.cmd).create_usage_with_title(&[]),
            ));
        }

        Ok(())
    }

    fn remove_overrides(&self, arg: &Arg, matcher: &mut ArgMatcher) {
        debug!("Parser::remove_overrides: id={:?}", arg.id);
        for override_id in &arg.overrides {
            debug!("Parser::remove_overrides:iter:{override_id:?}: removing");
            matcher.remove(override_id);
        }

        // Override anything that can override us
        let mut transitive = Vec::new();
        for arg_id in matcher.arg_ids() {
            if let Some(overrider) = self.cmd.find(arg_id) {
                if overrider.overrides.contains(arg.get_id()) {
                    transitive.push(overrider.get_id());
                }
            }
        }
        for overrider_id in transitive {
            debug!("Parser::remove_overrides:iter:{overrider_id:?}: removing");
            matcher.remove(overrider_id);
        }
    }

    #[cfg(feature = "env")]
    fn add_env(&mut self, matcher: &mut ArgMatcher) -> ClapResult<()> {
        debug!("Parser::add_env");

        for arg in self.cmd.get_arguments() {
            // Use env only if the arg was absent among command line args,
            // early return if this is not the case.
            if matcher.contains(&arg.id) {
                debug!("Parser::add_env: Skipping existing arg `{arg}`");
                continue;
            }

            debug!("Parser::add_env: Checking arg `{arg}`");
            if let Some((_, Some(ref val))) = arg.env {
                debug!("Parser::add_env: Found an opt with value={val:?}");
                let arg_values = vec![val.to_owned()];
                let trailing_idx = None;
                let _ = ok!(self.react(
                    None,
                    ValueSource::EnvVariable,
                    arg,
                    arg_values,
                    trailing_idx,
                    matcher,
                ));
            }
        }

        Ok(())
    }

    fn add_defaults(&self, matcher: &mut ArgMatcher) -> ClapResult<()> {
        debug!("Parser::add_defaults");

        for arg in self.cmd.get_arguments() {
            debug!("Parser::add_defaults:iter:{}:", arg.get_id());
            ok!(self.add_default_value(arg, matcher));
        }

        Ok(())
    }

    fn add_default_value(&self, arg: &Arg, matcher: &mut ArgMatcher) -> ClapResult<()> {
        if !arg.default_vals_ifs.is_empty() {
            debug!("Parser::add_default_value: has conditional defaults");
            if !matcher.contains(arg.get_id()) {
                for (id, val, default) in arg.default_vals_ifs.iter() {
                    let add = if let Some(a) = matcher.get(id) {
                        match val {
                            crate::builder::ArgPredicate::Equals(v) => {
                                a.raw_vals_flatten().any(|value| v == value)
                            }
                            crate::builder::ArgPredicate::IsPresent => true,
                        }
                    } else {
                        false
                    };

                    if add {
                        if let Some(default) = default {
                            let arg_values = vec![default.to_os_string()];
                            let trailing_idx = None;
                            let _ = ok!(self.react(
                                None,
                                ValueSource::DefaultValue,
                                arg,
                                arg_values,
                                trailing_idx,
                                matcher,
                            ));
                        }
                        return Ok(());
                    }
                }
            }
        } else {
            debug!("Parser::add_default_value: doesn't have conditional defaults");
        }

        if !arg.default_vals.is_empty() {
            debug!(
                "Parser::add_default_value:iter:{}: has default vals",
                arg.get_id()
            );
            if matcher.contains(arg.get_id()) {
                debug!("Parser::add_default_value:iter:{}: was used", arg.get_id());
            // do nothing
            } else {
                debug!(
                    "Parser::add_default_value:iter:{}: wasn't used",
                    arg.get_id()
                );
                let arg_values: Vec<_> = arg
                    .default_vals
                    .iter()
                    .map(crate::builder::OsStr::to_os_string)
                    .collect();
                let trailing_idx = None;
                let _ = ok!(self.react(
                    None,
                    ValueSource::DefaultValue,
                    arg,
                    arg_values,
                    trailing_idx,
                    matcher,
                ));
            }
        } else {
            debug!(
                "Parser::add_default_value:iter:{}: doesn't have default vals",
                arg.get_id()
            );

            // do nothing
        }

        Ok(())
    }

    fn start_custom_arg(&self, matcher: &mut ArgMatcher, arg: &Arg, source: ValueSource) {
        if source == ValueSource::CommandLine {
            // With each new occurrence, remove overrides from prior occurrences
            self.remove_overrides(arg, matcher);
        }
        matcher.start_custom_arg(arg, source);
        if source.is_explicit() {
            for group in self.cmd.groups_for_arg(arg.get_id()) {
                matcher.start_custom_group(group.clone(), source);
                matcher.add_val_to(
                    &group,
                    AnyValue::new(arg.get_id().clone()),
                    OsString::from(arg.get_id().as_str()),
                );
            }
        }
    }
}

// Error, Help, and Version Methods
impl Parser<'_> {
    /// Is only used for the long flag(which is the only one needs fuzzy searching)
    fn did_you_mean_error(
        &mut self,
        arg: &str,
        matcher: &mut ArgMatcher,
        remaining_args: &[&OsStr],
        trailing_values: bool,
    ) -> ClapError {
        debug!("Parser::did_you_mean_error: arg={arg}");
        // Didn't match a flag or option
        let longs = self
            .cmd
            .get_keymap()
            .keys()
            .filter_map(|x| match x {
                KeyType::Long(l) => Some(l.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Vec<_>>();
        debug!("Parser::did_you_mean_error: longs={longs:?}");

        let did_you_mean = suggestions::did_you_mean_flag(
            arg,
            remaining_args,
            longs.iter().map(|x| &x[..]),
            self.cmd.get_subcommands_mut(),
        );

        // Add the arg to the matches to build a proper usage string
        if !self.cmd.is_ignore_errors_set() {
            if let Some((name, _)) = did_you_mean.as_ref() {
                if let Some(arg) = self.cmd.get_keymap().get(&name.as_ref()) {
                    self.start_custom_arg(matcher, arg, ValueSource::CommandLine);
                }
            }
        }
        let did_you_mean = did_you_mean.map(|(arg, cmd)| (format!("--{arg}"), cmd));

        let required = self.cmd.required_graph();
        let used: Vec<Id> = matcher
            .arg_ids()
            .filter(|arg_id| {
                matcher.check_explicit(arg_id, &crate::builder::ArgPredicate::IsPresent)
            })
            .filter(|n| self.cmd.find(n).map(|a| !a.is_hide_set()).unwrap_or(false))
            .cloned()
            .collect();

        // `did_you_mean` is a lot more likely and should cause us to skip the `--` suggestion
        // with the one exception being that the CLI is trying to capture arguments
        //
        // In theory, this is only called for `--long`s, so we don't need to check
        let suggested_trailing_arg = (did_you_mean.is_none()
            || self
                .cmd
                .get_positionals()
                .any(|arg| arg.is_last_set() || arg.is_trailing_var_arg_set()))
            && !trailing_values
            && self.cmd.has_positionals();
        ClapError::unknown_argument(
            self.cmd,
            format!("--{arg}"),
            did_you_mean,
            suggested_trailing_arg,
            Usage::new(self.cmd)
                .required(&required)
                .create_usage_with_title(&used),
        )
    }

    fn help_err(&self, use_long: bool) -> ClapError {
        let styled = self.cmd.write_help_err(use_long);
        ClapError::display_help(self.cmd, styled)
    }

    fn version_err(&self, use_long: bool) -> ClapError {
        let styled = self.cmd.write_version_err(use_long);
        ClapError::display_version(self.cmd, styled)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParseState {
    ValuesDone,
    Opt(Id),
    Pos(Id),
}

/// Recoverable Parsing results.
#[derive(Debug, PartialEq, Clone)]
#[must_use]
enum ParseResult {
    FlagSubCommand(String),
    Opt(Id),
    ValuesDone,
    /// Value attached to the short flag is not consumed(e.g. 'u' for `-cu` is
    /// not consumed).
    AttachedValueNotConsumed,
    /// This long flag doesn't need a value but is provided one.
    UnneededAttachedValue {
        rest: String,
        used: Vec<Id>,
        arg: String,
    },
    /// This flag might be an hyphen Value.
    MaybeHyphenValue,
    /// Equals required but not provided.
    EqualsNotProvided {
        arg: String,
    },
    /// Failed to match a Arg.
    NoMatchingArg {
        arg: String,
    },
    /// No argument found e.g. parser is given `-` when parsing a flag.
    NoArg,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PendingArg {
    pub(crate) id: Id,
    pub(crate) ident: Option<Identifier>,
    pub(crate) raw_vals: Vec<OsString>,
    pub(crate) trailing_idx: Option<usize>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Identifier {
    Short,
    Long,
    Index,
}
