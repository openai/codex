/// Update action the CLI should perform after the TUI exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update via `npm install -g @openai/codex@latest`.
    NpmGlobalLatest,
    /// Update via `bun install -g @openai/codex@latest`.
    BunGlobalLatest,
    /// Update via `brew upgrade codex`.
    BrewUpgrade,
}

#[cfg(any(not(debug_assertions), test))]
pub(crate) fn get_update_action() -> Option<UpdateAction> {
    let exe = std::env::current_exe().unwrap_or_default();
    let managed_by_npm = std::env::var_os("CODEX_MANAGED_BY_NPM").is_some();
    let managed_by_bun = std::env::var_os("CODEX_MANAGED_BY_BUN").is_some();
    if managed_by_npm {
        Some(UpdateAction::NpmGlobalLatest)
    } else if managed_by_bun {
        Some(UpdateAction::BunGlobalLatest)
    } else if cfg!(target_os = "macos")
        && (exe.starts_with("/opt/homebrew") || exe.starts_with("/usr/local"))
    {
        Some(UpdateAction::BrewUpgrade)
    } else {
        None
    }
}

impl UpdateAction {
    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmGlobalLatest => ("npm", &["install", "-g", "@openai/codex@latest"]),
            UpdateAction::BunGlobalLatest => ("bun", &["install", "-g", "@openai/codex@latest"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "codex"]),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        let args_str = args.join(" ");
        format!("{command} {args_str}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_update_action() {
        let prev = std::env::var_os("CODEX_MANAGED_BY_NPM");

        // First: no npm var -> expect None (we do not run from brew in CI)
        unsafe { std::env::remove_var("CODEX_MANAGED_BY_NPM") };
        assert_eq!(get_update_action(), None);

        // Then: with npm var -> expect NpmGlobalLatest
        unsafe { std::env::set_var("CODEX_MANAGED_BY_NPM", "1") };
        assert_eq!(get_update_action(), Some(UpdateAction::NpmGlobalLatest));

        // Restore prior value to avoid leaking state
        if let Some(v) = prev {
            unsafe { std::env::set_var("CODEX_MANAGED_BY_NPM", v) };
        } else {
            unsafe { std::env::remove_var("CODEX_MANAGED_BY_NPM") };
        }
    }
}
