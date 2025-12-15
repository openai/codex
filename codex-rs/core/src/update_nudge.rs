use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateAction {
    NpmGlobalLatest,
    BunGlobalLatest,
    BrewUpgrade,
}

impl UpdateAction {
    fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "codex"]),
        }
    }

    fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

pub(crate) fn update_available_nudge() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let managed_by_npm = std::env::var_os("CODEX_MANAGED_BY_NPM").is_some();
    let managed_by_bun = std::env::var_os("CODEX_MANAGED_BY_BUN").is_some();
    let update_action = detect_update_action(
        cfg!(target_os = "macos"),
        &exe,
        managed_by_npm,
        managed_by_bun,
    );

    match update_action {
        Some(action) => {
            let command = action.command_str();
            format!("Update available. Run `{command}` to update.")
        }
        None => "Update available. See https://github.com/openai/codex for installation options."
            .to_string(),
    }
}

fn detect_update_action(
    is_macos: bool,
    current_exe: &Path,
    managed_by_npm: bool,
    managed_by_bun: bool,
) -> Option<UpdateAction> {
    if managed_by_npm {
        Some(UpdateAction::NpmGlobalLatest)
    } else if managed_by_bun {
        Some(UpdateAction::BunGlobalLatest)
    } else if is_macos
        && (current_exe.starts_with("/opt/homebrew") || current_exe.starts_with("/usr/local"))
    {
        Some(UpdateAction::BrewUpgrade)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_update_action_without_env_mutation() {
        assert_eq!(
            detect_update_action(false, Path::new("/any/path"), false, false),
            None
        );
        assert_eq!(
            detect_update_action(false, Path::new("/any/path"), true, false),
            Some(UpdateAction::NpmGlobalLatest)
        );
        assert_eq!(
            detect_update_action(false, Path::new("/any/path"), false, true),
            Some(UpdateAction::BunGlobalLatest)
        );
        assert_eq!(
            detect_update_action(true, Path::new("/opt/homebrew/bin/codex"), false, false),
            Some(UpdateAction::BrewUpgrade)
        );
        assert_eq!(
            detect_update_action(true, Path::new("/usr/local/bin/codex"), false, false),
            Some(UpdateAction::BrewUpgrade)
        );
    }
}
