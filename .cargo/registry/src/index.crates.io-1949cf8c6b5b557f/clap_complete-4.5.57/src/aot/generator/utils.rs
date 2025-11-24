//! Helpers for writing generators

use clap::{Arg, Command};

/// Gets all subcommands including child subcommands in the form of `("name", "bin_name")`.
///
/// Subcommand `rustup toolchain install` would be converted to
/// `("install", "rustup toolchain install")`.
pub fn all_subcommands(cmd: &Command) -> Vec<(String, String)> {
    let mut subcmds: Vec<_> = subcommands(cmd);

    for sc_v in cmd.get_subcommands().map(all_subcommands) {
        subcmds.extend(sc_v);
    }

    subcmds
}

/// Finds the subcommand [`clap::Command`] from the given [`clap::Command`] with the given path.
///
/// <div class="warning">
///
/// **NOTE:** `path` should not contain the root `bin_name`.
///
/// </div>
pub fn find_subcommand_with_path<'cmd>(p: &'cmd Command, path: Vec<&str>) -> &'cmd Command {
    let mut cmd = p;

    for sc in path {
        cmd = cmd.find_subcommand(sc).unwrap();
    }

    cmd
}

/// Gets subcommands of [`clap::Command`] in the form of `("name", "bin_name")`.
///
/// Subcommand `rustup toolchain install` would be converted to
/// `("install", "rustup toolchain install")`.
pub fn subcommands(p: &Command) -> Vec<(String, String)> {
    debug!("subcommands: name={}", p.get_name());
    debug!("subcommands: Has subcommands...{:?}", p.has_subcommands());

    let mut subcmds = vec![];

    for sc in p.get_subcommands() {
        let sc_bin_name = sc.get_bin_name().unwrap();

        debug!(
            "subcommands:iter: name={}, bin_name={}",
            sc.get_name(),
            sc_bin_name
        );
        subcmds.push((sc.get_name().to_string(), sc_bin_name.to_string()));

        for alias in sc.get_visible_aliases() {
            debug!(
                "subcommands:iter: alias={}, bin_name={}",
                alias, sc_bin_name
            );
            subcmds.push((alias.to_string(), sc_bin_name.to_string()));
        }
    }

    subcmds
}

/// Gets all the short options, their visible aliases and flags of a [`clap::Command`].
/// Includes `h` and `V` depending on the [`clap::Command`] settings.
pub fn shorts_and_visible_aliases(p: &Command) -> Vec<char> {
    debug!("shorts: name={}", p.get_name());

    p.get_arguments()
        .filter_map(|a| {
            if !a.is_positional() {
                if a.get_visible_short_aliases().is_some() && a.get_short().is_some() {
                    let mut shorts_and_visible_aliases = a.get_visible_short_aliases().unwrap();
                    shorts_and_visible_aliases.push(a.get_short().unwrap());
                    Some(shorts_and_visible_aliases)
                } else if a.get_visible_short_aliases().is_none() && a.get_short().is_some() {
                    Some(vec![a.get_short().unwrap()])
                } else {
                    None
                }
            } else {
                None
            }
        })
        .flatten()
        .collect()
}

/// Gets all the long options, their visible aliases and flags of a [`clap::Command`].
/// Includes `help` and `version` depending on the [`clap::Command`] settings.
pub fn longs_and_visible_aliases(p: &Command) -> Vec<String> {
    debug!("longs: name={}", p.get_name());

    p.get_arguments()
        .filter_map(|a| {
            if !a.is_positional() {
                if a.get_visible_aliases().is_some() && a.get_long().is_some() {
                    let mut visible_aliases: Vec<_> = a
                        .get_visible_aliases()
                        .unwrap()
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect();
                    visible_aliases.push(a.get_long().unwrap().to_string());
                    Some(visible_aliases)
                } else if a.get_visible_aliases().is_none() && a.get_long().is_some() {
                    Some(vec![a.get_long().unwrap().to_string()])
                } else {
                    None
                }
            } else {
                None
            }
        })
        .flatten()
        .collect()
}

/// Gets all the flags of a [`clap::Command`].
/// Includes `help` and `version` depending on the [`clap::Command`] settings.
pub fn flags(p: &Command) -> Vec<Arg> {
    debug!("flags: name={}", p.get_name());
    p.get_arguments()
        .filter(|a| !a.get_num_args().expect("built").takes_values() && !a.is_positional())
        .cloned()
        .collect()
}

/// Get the possible values for completion
pub fn possible_values(a: &Arg) -> Option<Vec<clap::builder::PossibleValue>> {
    if !a.get_num_args().expect("built").takes_values() {
        None
    } else {
        a.get_value_parser()
            .possible_values()
            .map(|pvs| pvs.collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Arg;
    use clap::ArgAction;

    fn common_app() -> Command {
        Command::new("myapp")
            .subcommand(
                Command::new("test").subcommand(Command::new("config")).arg(
                    Arg::new("file")
                        .short('f')
                        .short_alias('c')
                        .visible_short_alias('p')
                        .long("file")
                        .action(ArgAction::SetTrue)
                        .visible_alias("path"),
                ),
            )
            .subcommand(Command::new("hello"))
            .bin_name("my-cmd")
    }

    fn built() -> Command {
        let mut cmd = common_app();

        cmd.build();
        cmd
    }

    fn built_with_version() -> Command {
        let mut cmd = common_app().version("3.0");

        cmd.build();
        cmd
    }

    #[test]
    fn test_subcommands() {
        let cmd = built_with_version();

        assert_eq!(
            subcommands(&cmd),
            vec![
                ("test".to_string(), "my-cmd test".to_string()),
                ("hello".to_string(), "my-cmd hello".to_string()),
                ("help".to_string(), "my-cmd help".to_string()),
            ]
        );
    }

    #[test]
    fn test_all_subcommands() {
        let cmd = built_with_version();

        assert_eq!(
            all_subcommands(&cmd),
            vec![
                ("test".to_string(), "my-cmd test".to_string()),
                ("hello".to_string(), "my-cmd hello".to_string()),
                ("help".to_string(), "my-cmd help".to_string()),
                ("config".to_string(), "my-cmd test config".to_string()),
                ("help".to_string(), "my-cmd test help".to_string()),
                ("config".to_string(), "my-cmd test help config".to_string()),
                ("help".to_string(), "my-cmd test help help".to_string()),
                ("test".to_string(), "my-cmd help test".to_string()),
                ("hello".to_string(), "my-cmd help hello".to_string()),
                ("help".to_string(), "my-cmd help help".to_string()),
                ("config".to_string(), "my-cmd help test config".to_string()),
            ]
        );
    }

    #[test]
    fn test_find_subcommand_with_path() {
        let cmd = built_with_version();
        let sc_app = find_subcommand_with_path(&cmd, "test config".split(' ').collect());

        assert_eq!(sc_app.get_name(), "config");
    }

    #[test]
    fn test_flags() {
        let cmd = built_with_version();
        let actual_flags = flags(&cmd);

        assert_eq!(actual_flags.len(), 2);
        assert_eq!(actual_flags[0].get_long(), Some("help"));
        assert_eq!(actual_flags[1].get_long(), Some("version"));

        let sc_flags = flags(find_subcommand_with_path(&cmd, vec!["test"]));

        assert_eq!(sc_flags.len(), 2);
        assert_eq!(sc_flags[0].get_long(), Some("file"));
        assert_eq!(sc_flags[1].get_long(), Some("help"));
    }

    #[test]
    fn test_flag_subcommand() {
        let cmd = built();
        let actual_flags = flags(&cmd);

        assert_eq!(actual_flags.len(), 1);
        assert_eq!(actual_flags[0].get_long(), Some("help"));

        let sc_flags = flags(find_subcommand_with_path(&cmd, vec!["test"]));

        assert_eq!(sc_flags.len(), 2);
        assert_eq!(sc_flags[0].get_long(), Some("file"));
        assert_eq!(sc_flags[1].get_long(), Some("help"));
    }

    #[test]
    fn test_shorts() {
        let cmd = built_with_version();
        let shorts = shorts_and_visible_aliases(&cmd);

        assert_eq!(shorts.len(), 2);
        assert_eq!(shorts[0], 'h');
        assert_eq!(shorts[1], 'V');

        let sc_shorts = shorts_and_visible_aliases(find_subcommand_with_path(&cmd, vec!["test"]));

        assert_eq!(sc_shorts.len(), 3);
        assert_eq!(sc_shorts[0], 'p');
        assert_eq!(sc_shorts[1], 'f');
        assert_eq!(sc_shorts[2], 'h');
    }

    #[test]
    fn test_longs() {
        let cmd = built_with_version();
        let longs = longs_and_visible_aliases(&cmd);

        assert_eq!(longs.len(), 2);
        assert_eq!(longs[0], "help");
        assert_eq!(longs[1], "version");

        let sc_longs = longs_and_visible_aliases(find_subcommand_with_path(&cmd, vec!["test"]));

        assert_eq!(sc_longs.len(), 3);
        assert_eq!(sc_longs[0], "path");
        assert_eq!(sc_longs[1], "file");
        assert_eq!(sc_longs[2], "help");
    }
}
