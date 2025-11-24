use std::cmp::Ordering;

use crate::builder::ValueRange;
use crate::mkeymap::KeyType;
use crate::util::FlatSet;
use crate::util::Id;
use crate::ArgAction;
use crate::INTERNAL_ERROR_MSG;
use crate::{Arg, Command, ValueHint};

pub(crate) fn assert_app(cmd: &Command) {
    debug!("Command::_debug_asserts");

    let mut short_flags = vec![];
    let mut long_flags = vec![];

    // Invalid version flag settings
    if cmd.get_version().is_none() && cmd.get_long_version().is_none() {
        // PropagateVersion is meaningless if there is no version
        assert!(
            !cmd.is_propagate_version_set(),
            "Command {}: No version information via Command::version or Command::long_version to propagate",
            cmd.get_name(),
        );

        // Used `Command::mut_arg("version", ..) but did not provide any version information to display
        let version_needed = cmd
            .get_arguments()
            .filter(|x| matches!(x.get_action(), ArgAction::Version))
            .map(|x| x.get_id())
            .collect::<Vec<_>>();

        assert_eq!(version_needed, Vec::<&str>::new(), "Command {}: `ArgAction::Version` used without providing Command::version or Command::long_version"
            ,cmd.get_name()
        );
    }

    for sc in cmd.get_subcommands() {
        if let Some(s) = sc.get_short_flag().as_ref() {
            short_flags.push(Flag::Command(format!("-{s}"), sc.get_name()));
        }

        for short_alias in sc.get_all_short_flag_aliases() {
            short_flags.push(Flag::Command(format!("-{short_alias}"), sc.get_name()));
        }

        if let Some(l) = sc.get_long_flag().as_ref() {
            assert!(!l.starts_with('-'), "Command {}: long_flag {:?} must not start with a `-`, that will be handled by the parser", sc.get_name(), l);
            long_flags.push(Flag::Command(format!("--{l}"), sc.get_name()));
        }

        for long_alias in sc.get_all_long_flag_aliases() {
            long_flags.push(Flag::Command(format!("--{long_alias}"), sc.get_name()));
        }
    }

    for arg in cmd.get_arguments() {
        assert_arg(arg);

        assert!(
            !cmd.is_multicall_set(),
            "Command {}: Arguments like {} cannot be set on a multicall command",
            cmd.get_name(),
            arg.get_id()
        );

        if let Some(s) = arg.get_short() {
            short_flags.push(Flag::Arg(format!("-{s}"), arg.get_id().as_str()));
        }

        for (short_alias, _) in &arg.short_aliases {
            short_flags.push(Flag::Arg(format!("-{short_alias}"), arg.get_id().as_str()));
        }

        if let Some(l) = arg.get_long() {
            assert!(!l.starts_with('-'), "Argument {}: long {:?} must not start with a `-`, that will be handled by the parser", arg.get_id(), l);
            long_flags.push(Flag::Arg(format!("--{l}"), arg.get_id().as_str()));
        }

        for (long_alias, _) in &arg.aliases {
            long_flags.push(Flag::Arg(format!("--{long_alias}"), arg.get_id().as_str()));
        }

        // Name conflicts
        if let Some((first, second)) = cmd.two_args_of(|x| x.get_id() == arg.get_id()) {
            panic!(
            "Command {}: Argument names must be unique, but '{}' is in use by more than one argument or group{}",
            cmd.get_name(),
            arg.get_id(),
            duplicate_tip(cmd, first, second),
        );
        }

        // Long conflicts
        if let Some(l) = arg.get_long() {
            if let Some((first, second)) = cmd.two_args_of(|x| x.get_long() == Some(l)) {
                panic!(
                    "Command {}: Long option names must be unique for each argument, \
                            but '--{}' is in use by both '{}' and '{}'{}",
                    cmd.get_name(),
                    l,
                    first.get_id(),
                    second.get_id(),
                    duplicate_tip(cmd, first, second)
                )
            }
        }

        // Short conflicts
        if let Some(s) = arg.get_short() {
            if let Some((first, second)) = cmd.two_args_of(|x| x.get_short() == Some(s)) {
                panic!(
                    "Command {}: Short option names must be unique for each argument, \
                            but '-{}' is in use by both '{}' and '{}'{}",
                    cmd.get_name(),
                    s,
                    first.get_id(),
                    second.get_id(),
                    duplicate_tip(cmd, first, second),
                )
            }
        }

        // Index conflicts
        if let Some(idx) = arg.index {
            if let Some((first, second)) =
                cmd.two_args_of(|x| x.is_positional() && x.get_index() == Some(idx))
            {
                panic!(
                    "Command {}: Argument '{}' has the same index as '{}' \
                    and they are both positional arguments\n\n\t \
                    Use `Arg::num_args(1..)` to allow one \
                    positional argument to take multiple values",
                    cmd.get_name(),
                    first.get_id(),
                    second.get_id()
                )
            }
        }

        // requires, r_if, r_unless
        for (_predicate, req_id) in &arg.requires {
            assert!(
                &arg.id != req_id,
                "Argument {} cannot require itself",
                arg.get_id()
            );

            assert!(
                cmd.id_exists(req_id),
                "Command {}: Argument or group '{}' specified in 'requires*' for '{}' does not exist",
                cmd.get_name(),
                req_id,
                arg.get_id(),
            );
        }

        for req in &arg.r_ifs {
            assert!(
                !arg.is_required_set(),
                "Argument {}: `required` conflicts with `required_if_eq*`",
                arg.get_id()
            );
            assert!(
                cmd.id_exists(&req.0),
                "Command {}: Argument or group '{}' specified in 'required_if_eq*' for '{}' does not exist",
                    cmd.get_name(),
                req.0,
                arg.get_id()
            );
        }

        for req in &arg.r_ifs_all {
            assert!(
                !arg.is_required_set(),
                "Argument {}: `required` conflicts with `required_if_eq_all`",
                arg.get_id()
            );
            assert!(
                cmd.id_exists(&req.0),
                "Command {}: Argument or group '{}' specified in 'required_if_eq_all' for '{}' does not exist",
                    cmd.get_name(),
                req.0,
                arg.get_id()
            );
        }

        for req in &arg.r_unless {
            assert!(
                !arg.is_required_set(),
                "Argument {}: `required` conflicts with `required_unless*`",
                arg.get_id()
            );
            assert!(
                cmd.id_exists(req),
                "Command {}: Argument or group '{}' specified in 'required_unless*' for '{}' does not exist",
                    cmd.get_name(),
                req,
                arg.get_id(),
            );
        }

        for req in &arg.r_unless_all {
            assert!(
                !arg.is_required_set(),
                "Argument {}: `required` conflicts with `required_unless*`",
                arg.get_id()
            );
            assert!(
                cmd.id_exists(req),
                "Command {}: Argument or group '{}' specified in 'required_unless*' for '{}' does not exist",
                    cmd.get_name(),
                req,
                arg.get_id(),
            );
        }

        // blacklist
        for req in &arg.blacklist {
            assert!(
                cmd.id_exists(req),
                "Command {}: Argument or group '{}' specified in 'conflicts_with*' for '{}' does not exist",
                    cmd.get_name(),
                req,
                arg.get_id(),
            );
        }

        // overrides
        for req in &arg.overrides {
            assert!(
                cmd.id_exists(req),
                "Command {}: Argument or group '{}' specified in 'overrides_with*' for '{}' does not exist",
                    cmd.get_name(),
                req,
                arg.get_id(),
            );
        }

        if arg.is_last_set() {
            assert!(
                arg.get_long().is_none(),
                "Command {}: Flags or Options cannot have last(true) set. '{}' has both a long and last(true) set.",
                    cmd.get_name(),
                arg.get_id()
            );
            assert!(
                arg.get_short().is_none(),
                "Command {}: Flags or Options cannot have last(true) set. '{}' has both a short and last(true) set.",
                    cmd.get_name(),
                arg.get_id()
            );
        }

        assert!(
            !(arg.is_required_set() && arg.is_global_set()),
            "Command {}: Global arguments cannot be required.\n\n\t'{}' is marked as both global and required",
                    cmd.get_name(),
            arg.get_id()
        );

        if arg.get_value_hint() == ValueHint::CommandWithArguments {
            assert!(
                arg.is_positional(),
                "Command {}: Argument '{}' has hint CommandWithArguments and must be positional.",
                cmd.get_name(),
                arg.get_id()
            );

            assert!(
                arg.is_trailing_var_arg_set() || arg.is_last_set(),
                "Command {}: Positional argument '{}' has hint CommandWithArguments, so Command must have `trailing_var_arg(true)` or `last(true)` set.",
                    cmd.get_name(),
                arg.get_id()
            );
        }
    }

    for group in cmd.get_groups() {
        // Name conflicts
        assert!(
            cmd.get_groups().filter(|x| x.id == group.id).count() < 2,
            "Command {}: Argument group name must be unique\n\n\t'{}' is already in use",
            cmd.get_name(),
            group.get_id(),
        );

        // Groups should not have naming conflicts with Args
        assert!(
            !cmd.get_arguments().any(|x| x.get_id() == group.get_id()),
            "Command {}: Argument group name '{}' must not conflict with argument name",
            cmd.get_name(),
            group.get_id(),
        );

        for arg in &group.args {
            // Args listed inside groups should exist
            assert!(
                cmd.get_arguments().any(|x| x.get_id() == arg),
                "Command {}: Argument group '{}' contains non-existent argument '{}'",
                cmd.get_name(),
                group.get_id(),
                arg
            );
        }

        for arg in &group.requires {
            // Args listed inside groups should exist
            assert!(
                cmd.id_exists(arg),
                "Command {}: Argument group '{}' requires non-existent '{}' id",
                cmd.get_name(),
                group.get_id(),
                arg
            );
        }

        for arg in &group.conflicts {
            // Args listed inside groups should exist
            assert!(
                cmd.id_exists(arg),
                "Command {}: Argument group '{}' conflicts with non-existent '{}' id",
                cmd.get_name(),
                group.get_id(),
                arg
            );
        }
    }

    // Conflicts between flags and subcommands

    long_flags.sort_unstable();
    short_flags.sort_unstable();

    detect_duplicate_flags(&long_flags, "long");
    detect_duplicate_flags(&short_flags, "short");

    let mut subs = FlatSet::new();
    for sc in cmd.get_subcommands() {
        assert!(
            subs.insert(sc.get_name()),
            "Command {}: command name `{}` is duplicated",
            cmd.get_name(),
            sc.get_name()
        );
        for alias in sc.get_all_aliases() {
            assert!(
                subs.insert(alias),
                "Command {}: command `{}` alias `{}` is duplicated",
                cmd.get_name(),
                sc.get_name(),
                alias
            );
        }
    }

    _verify_positionals(cmd);

    #[cfg(feature = "help")]
    if let Some(help_template) = cmd.get_help_template() {
        assert!(
            !help_template.to_string().contains("{flags}"),
            "Command {}: {}",
                    cmd.get_name(),
            "`{flags}` template variable was removed in clap3, they are now included in `{options}`",
        );
        assert!(
            !help_template.to_string().contains("{unified}"),
            "Command {}: {}",
            cmd.get_name(),
            "`{unified}` template variable was removed in clap3, use `{options}` instead"
        );
        #[cfg(feature = "unstable-v5")]
        assert!(
            !help_template.to_string().contains("{bin}"),
            "Command {}: {}",
            cmd.get_name(),
            "`{bin}` template variable was removed in clap5, use `{name}` instead"
        );
    }

    cmd._panic_on_missing_help(cmd.is_help_expected_set());
    assert_app_flags(cmd);
}

fn duplicate_tip(cmd: &Command, first: &Arg, second: &Arg) -> &'static str {
    if !cmd.is_disable_help_flag_set()
        && (first.get_id() == Id::HELP || second.get_id() == Id::HELP)
    {
        " (call `cmd.disable_help_flag(true)` to remove the auto-generated `--help`)"
    } else if !cmd.is_disable_version_flag_set()
        && (first.get_id() == Id::VERSION || second.get_id() == Id::VERSION)
    {
        " (call `cmd.disable_version_flag(true)` to remove the auto-generated `--version`)"
    } else {
        ""
    }
}

#[derive(Eq)]
enum Flag<'a> {
    Command(String, &'a str),
    Arg(String, &'a str),
}

impl PartialEq for Flag<'_> {
    fn eq(&self, other: &Flag<'_>) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl PartialOrd for Flag<'_> {
    fn partial_cmp(&self, other: &Flag<'_>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Flag<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Flag::Command(s1, _), Flag::Command(s2, _))
            | (Flag::Arg(s1, _), Flag::Arg(s2, _))
            | (Flag::Command(s1, _), Flag::Arg(s2, _))
            | (Flag::Arg(s1, _), Flag::Command(s2, _)) => {
                if s1 == s2 {
                    Ordering::Equal
                } else {
                    s1.cmp(s2)
                }
            }
        }
    }
}

fn detect_duplicate_flags(flags: &[Flag<'_>], short_or_long: &str) {
    for (one, two) in find_duplicates(flags) {
        match (one, two) {
            (Flag::Command(flag, one), Flag::Command(_, another)) if one != another => panic!(
                "the '{flag}' {short_or_long} flag is specified for both '{one}' and '{another}' subcommands"
            ),

            (Flag::Arg(flag, one), Flag::Arg(_, another)) if one != another => panic!(
                "{short_or_long} option names must be unique, but '{flag}' is in use by both '{one}' and '{another}'"
            ),

            (Flag::Arg(flag, arg), Flag::Command(_, sub)) | (Flag::Command(flag, sub), Flag::Arg(_, arg)) => panic!(
                "the '{flag}' {short_or_long} flag for the '{arg}' argument conflicts with the short flag \
                     for '{sub}' subcommand"
            ),

            _ => {}
        }
    }
}

/// Find duplicates in a sorted array.
///
/// The algorithm is simple: the array is sorted, duplicates
/// must be placed next to each other, we can check only adjacent elements.
fn find_duplicates<T: PartialEq>(slice: &[T]) -> impl Iterator<Item = (&T, &T)> {
    slice.windows(2).filter_map(|w| {
        if w[0] == w[1] {
            Some((&w[0], &w[1]))
        } else {
            None
        }
    })
}

fn assert_app_flags(cmd: &Command) {
    macro_rules! checker {
        ($a:ident conflicts $($b:ident)|+) => {
            if cmd.$a() {
                let mut s = String::new();

                $(
                    if cmd.$b() {
                        use std::fmt::Write;
                        write!(&mut s, "  AppSettings::{} conflicts with AppSettings::{}.\n", std::stringify!($b), std::stringify!($a)).unwrap();
                    }
                )+

                if !s.is_empty() {
                    panic!("{}\n{}", cmd.get_name(), s)
                }
            }
        };
    }

    checker!(is_multicall_set conflicts is_no_binary_name_set);
}

#[cfg(debug_assertions)]
fn _verify_positionals(cmd: &Command) -> bool {
    debug!("Command::_verify_positionals");
    // Because you must wait until all arguments have been supplied, this is the first chance
    // to make assertions on positional argument indexes
    //
    // First we verify that the index highest supplied index, is equal to the number of
    // positional arguments to verify there are no gaps (i.e. supplying an index of 1 and 3
    // but no 2)

    let highest_idx = cmd
        .get_keymap()
        .keys()
        .filter_map(|x| {
            if let KeyType::Position(n) = x {
                Some(*n)
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0);

    let num_p = cmd.get_keymap().keys().filter(|x| x.is_position()).count();

    assert!(
        highest_idx == num_p,
        "Found positional argument whose index is {highest_idx} but there \
             are only {num_p} positional arguments defined",
    );

    for arg in cmd.get_arguments() {
        if arg.index.unwrap_or(0) == highest_idx {
            assert!(
                !arg.is_trailing_var_arg_set() || !arg.is_last_set(),
                "{}:{}: `Arg::trailing_var_arg` and `Arg::last` cannot be used together",
                cmd.get_name(),
                arg.get_id()
            );

            if arg.is_trailing_var_arg_set() {
                assert!(
                    arg.is_multiple(),
                    "{}:{}: `Arg::trailing_var_arg` must accept multiple values",
                    cmd.get_name(),
                    arg.get_id()
                );
            }
        } else {
            assert!(
                !arg.is_trailing_var_arg_set(),
                "{}:{}: `Arg::trailing_var_arg` can only apply to last positional",
                cmd.get_name(),
                arg.get_id()
            );
        }
    }

    // Next we verify that only the highest index has takes multiple arguments (if any)
    let only_highest = |a: &Arg| a.is_multiple() && (a.get_index().unwrap_or(0) != highest_idx);
    if cmd.get_positionals().any(only_highest) {
        // First we make sure if there is a positional that allows multiple values
        // the one before it (second to last) has one of these:
        //  * a value terminator
        //  * ArgSettings::Last
        //  * The last arg is Required

        // We can't pass the closure (it.next()) to the macro directly because each call to
        // find() (iterator, not macro) gets called repeatedly.
        let last = &cmd.get_keymap()[&KeyType::Position(highest_idx)];
        let second_to_last = &cmd.get_keymap()[&KeyType::Position(highest_idx - 1)];

        // Either the final positional is required
        // Or the second to last has a terminator or .last(true) set
        let ok = last.is_required_set()
            || (second_to_last.terminator.is_some() || second_to_last.is_last_set())
            || last.is_last_set();
        assert!(
            ok,
            "Positional argument `{last}` *must* have `required(true)` or `last(true)` set \
            because a prior positional argument (`{second_to_last}`) has `num_args(1..)`"
        );

        // We make sure if the second to last is Multiple the last is ArgSettings::Last
        let ok = second_to_last.is_multiple() || last.is_last_set();
        assert!(
            ok,
            "Only the last positional argument, or second to last positional \
                 argument may be set to `.num_args(1..)`"
        );

        // Next we check how many have both Multiple and not a specific number of values set
        let count = cmd
            .get_positionals()
            .filter(|p| {
                p.is_multiple_values_set()
                    && p.get_value_terminator().is_none()
                    && !p.get_num_args().expect(INTERNAL_ERROR_MSG).is_fixed()
            })
            .count();
        let ok = count <= 1
            || (last.is_last_set()
                && last.is_multiple()
                && second_to_last.is_multiple()
                && count == 2);
        assert!(
            ok,
            "Only one positional argument with `.num_args(1..)` set is allowed per \
                 command, unless the second one also has .last(true) set"
        );
    }

    let mut found = false;

    if cmd.is_allow_missing_positional_set() {
        // Check that if a required positional argument is found, all positions with a lower
        // index are also required.
        let mut foundx2 = false;

        for p in cmd.get_positionals() {
            if foundx2 && !p.is_required_set() {
                assert!(
                    p.is_required_set(),
                    "Found non-required positional argument with a lower \
                         index than a required positional argument by two or more: {:?} \
                         index {:?}",
                    p.get_id(),
                    p.get_index()
                );
            } else if p.is_required_set() && !p.is_last_set() {
                // Args that .last(true) don't count since they can be required and have
                // positionals with a lower index that aren't required
                // Imagine: prog <req1> [opt1] -- <req2>
                // Both of these are valid invocations:
                //      $ prog r1 -- r2
                //      $ prog r1 o1 -- r2
                if found {
                    foundx2 = true;
                    continue;
                }
                found = true;
            } else {
                found = false;
            }
        }
    } else {
        // Check that if a required positional argument is found, all positions with a lower
        // index are also required
        for p in (1..=num_p).rev().filter_map(|n| cmd.get_keymap().get(&n)) {
            if found {
                assert!(
                    p.is_required_set(),
                    "Found non-required positional argument with a lower \
                         index than a required positional argument: {:?} index {:?}",
                    p.get_id(),
                    p.get_index()
                );
            } else if p.is_required_set() && !p.is_last_set() {
                // Args that .last(true) don't count since they can be required and have
                // positionals with a lower index that aren't required
                // Imagine: prog <req1> [opt1] -- <req2>
                // Both of these are valid invocations:
                //      $ prog r1 -- r2
                //      $ prog r1 o1 -- r2
                found = true;
            }
        }
    }
    assert!(
        cmd.get_positionals().filter(|p| p.is_last_set()).count() < 2,
        "Only one positional argument may have last(true) set. Found two."
    );
    if cmd
        .get_positionals()
        .any(|p| p.is_last_set() && p.is_required_set())
        && cmd.has_subcommands()
        && !cmd.is_subcommand_negates_reqs_set()
    {
        panic!(
            "Having a required positional argument with .last(true) set *and* child \
                 subcommands without setting SubcommandsNegateReqs isn't compatible."
        );
    }

    true
}

fn assert_arg(arg: &Arg) {
    debug!("Arg::_debug_asserts:{}", arg.get_id());

    // Self conflict
    // TODO: this check should be recursive
    assert!(
        !arg.blacklist.iter().any(|x| x == arg.get_id()),
        "Argument '{}' cannot conflict with itself",
        arg.get_id(),
    );

    assert!(
        arg.get_num_args().unwrap_or(1.into()).max_values()
            <= arg.get_action().max_num_args().max_values(),
        "Argument `{}`'s action {:?} is incompatible with `num_args({:?})`",
        arg.get_id(),
        arg.get_action(),
        arg.get_num_args().unwrap_or(1.into())
    );
    if let Some(action_type_id) = arg.get_action().value_type_id() {
        assert_eq!(
            action_type_id,
            arg.get_value_parser().type_id(),
            "Argument `{}`'s selected action {:?} contradicts `value_parser` ({:?})",
            arg.get_id(),
            arg.get_action(),
            arg.get_value_parser()
        );
    }

    if arg.get_value_hint() != ValueHint::Unknown {
        assert!(
            arg.is_takes_value_set(),
            "Argument '{}' has value hint but takes no value",
            arg.get_id()
        );

        if arg.get_value_hint() == ValueHint::CommandWithArguments {
            assert!(
                arg.is_multiple_values_set(),
                "Argument '{}' uses hint CommandWithArguments and must accept multiple values",
                arg.get_id()
            );
        }
    }

    if arg.index.is_some() {
        assert!(
            arg.is_positional(),
            "Argument '{}' is a positional argument and can't have short or long name versions",
            arg.get_id()
        );
        assert!(
            arg.is_takes_value_set(),
            "Argument '{}' is positional and it must take a value but action is {:?}{}",
            arg.get_id(),
            arg.get_action(),
            if arg.get_id() == Id::HELP {
                " (`mut_arg` no longer works with implicit `--help`)"
            } else if arg.get_id() == Id::VERSION {
                " (`mut_arg` no longer works with implicit `--version`)"
            } else {
                ""
            }
        );
    }

    let num_vals = arg.get_num_args().expect(INTERNAL_ERROR_MSG);
    // This can be the cause of later asserts, so put this first
    if num_vals != ValueRange::EMPTY {
        // HACK: Don't check for flags to make the derive easier
        let num_val_names = arg.get_value_names().unwrap_or(&[]).len();
        if num_vals.max_values() < num_val_names {
            panic!(
                "Argument {}: Too many value names ({}) compared to `num_args` ({})",
                arg.get_id(),
                num_val_names,
                num_vals
            );
        }
    }

    assert_eq!(
        num_vals.is_multiple(),
        arg.is_multiple_values_set(),
        "Argument {}: mismatch between `num_args` ({}) and `multiple_values`",
        arg.get_id(),
        num_vals,
    );

    if 1 < num_vals.min_values() {
        assert!(
            !arg.is_require_equals_set(),
            "Argument {}: cannot accept more than 1 arg (num_args={}) with require_equals",
            arg.get_id(),
            num_vals
        );
    }

    if num_vals == ValueRange::SINGLE {
        assert!(
            !arg.is_multiple_values_set(),
            "Argument {}: mismatch between `num_args` and `multiple_values`",
            arg.get_id()
        );
    }

    assert_arg_flags(arg);
}

fn assert_arg_flags(arg: &Arg) {
    macro_rules! checker {
        ($a:ident requires $($b:ident)|+) => {
            if arg.$a() {
                let mut s = String::new();

                $(
                    if !arg.$b() {
                        use std::fmt::Write;
                        write!(&mut s, "  Arg::{} is required when Arg::{} is set.\n", std::stringify!($b), std::stringify!($a)).unwrap();
                    }
                )+

                if !s.is_empty() {
                    panic!("Argument {:?}\n{}", arg.get_id(), s)
                }
            }
        }
    }

    checker!(is_hide_possible_values_set requires is_takes_value_set);
    checker!(is_allow_hyphen_values_set requires is_takes_value_set);
    checker!(is_allow_negative_numbers_set requires is_takes_value_set);
    checker!(is_require_equals_set requires is_takes_value_set);
    checker!(is_last_set requires is_takes_value_set);
    checker!(is_hide_default_value_set requires is_takes_value_set);
    checker!(is_multiple_values_set requires is_takes_value_set);
    checker!(is_ignore_case_set requires is_takes_value_set);
}
