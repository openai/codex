#[cfg(not(debug_assertions))]
const CODEX_MANAGED_BY_NPM_ENV_VAR: &str = "CODEX_MANAGED_BY_NPM";
#[cfg(not(debug_assertions))]
const CODEX_MANAGED_BY_BUN_ENV_VAR: &str = "CODEX_MANAGED_BY_BUN";

/// Update action the CLI should perform after the TUI exits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateAction {
    /// Update via `npm install -g @ixe1/codexel@latest`.
    NpmUpgrade,
    /// Update via `bun install -g @ixe1/codexel@latest`.
    BunUpgrade,
    /// Update via `brew upgrade --cask codexel`.
    BrewUpgrade,
}

impl From<UpdateAction> for codex_tui::update_action::UpdateAction {
    fn from(action: UpdateAction) -> Self {
        match action {
            UpdateAction::NpmUpgrade => codex_tui::update_action::UpdateAction::NpmUpgrade,
            UpdateAction::BunUpgrade => codex_tui::update_action::UpdateAction::BunUpgrade,
            UpdateAction::BrewUpgrade => codex_tui::update_action::UpdateAction::BrewUpgrade,
        }
    }
}

impl UpdateAction {
    /// Returns the list of command-line arguments for invoking the update.
    pub fn command_args(self) -> (&'static str, &'static [&'static str]) {
        match self {
            UpdateAction::NpmUpgrade => ("npm", &["install", "-g", "@ixe1/codexel@latest"]),
            UpdateAction::BunUpgrade => ("bun", &["install", "-g", "@ixe1/codexel@latest"]),
            UpdateAction::BrewUpgrade => ("brew", &["upgrade", "--cask", "codexel"]),
        }
    }

    /// Returns string representation of the command-line arguments for invoking the update.
    pub fn command_str(self) -> String {
        let (command, args) = self.command_args();
        shlex::try_join(std::iter::once(command).chain(args.iter().copied()))
            .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
    }
}

#[cfg(not(debug_assertions))]
pub(crate) fn get_update_action() -> Option<UpdateAction> {
    let exe = std::env::current_exe().unwrap_or_default();

    detect_update_action(cfg!(target_os = "macos"), &exe, ManagedBy::from_env())
}

#[cfg(any(not(debug_assertions), test))]
fn detect_update_action(
    is_macos: bool,
    current_exe: &std::path::Path,
    managed_by: Option<ManagedBy>,
) -> Option<UpdateAction> {
    if let Some(managed_by) = managed_by {
        return Some(managed_by.to_update_action());
    }
    if is_macos
        && (current_exe.starts_with("/opt/homebrew") || current_exe.starts_with("/usr/local"))
    {
        Some(UpdateAction::BrewUpgrade)
    } else {
        None
    }
}

#[cfg(any(not(debug_assertions), test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedBy {
    Npm,
    Bun,
}

#[cfg(any(not(debug_assertions), test))]
impl ManagedBy {
    #[cfg(not(debug_assertions))]
    fn from_env() -> Option<Self> {
        if std::env::var_os(CODEX_MANAGED_BY_BUN_ENV_VAR).is_some() {
            return Some(Self::Bun);
        }
        if std::env::var_os(CODEX_MANAGED_BY_NPM_ENV_VAR).is_some() {
            return Some(Self::Npm);
        }
        None
    }

    fn to_update_action(self) -> UpdateAction {
        match self {
            ManagedBy::Npm => UpdateAction::NpmUpgrade,
            ManagedBy::Bun => UpdateAction::BunUpgrade,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_update_action_without_env_mutation() {
        assert_eq!(
            detect_update_action(false, std::path::Path::new("/any/path"), None),
            None
        );
        assert_eq!(
            detect_update_action(
                true,
                std::path::Path::new("/opt/homebrew/bin/codexel"),
                None
            ),
            Some(UpdateAction::BrewUpgrade)
        );
        assert_eq!(
            detect_update_action(true, std::path::Path::new("/usr/local/bin/codexel"), None),
            Some(UpdateAction::BrewUpgrade)
        );
    }

    #[test]
    fn detects_update_action_from_package_manager() {
        assert_eq!(
            detect_update_action(
                false,
                std::path::Path::new("/any/path"),
                Some(ManagedBy::Npm)
            ),
            Some(UpdateAction::NpmUpgrade)
        );
        assert_eq!(
            detect_update_action(
                false,
                std::path::Path::new("/any/path"),
                Some(ManagedBy::Bun)
            ),
            Some(UpdateAction::BunUpgrade)
        );
        assert_eq!(
            detect_update_action(
                true,
                std::path::Path::new("/opt/homebrew/bin/codexel"),
                Some(ManagedBy::Npm)
            ),
            Some(UpdateAction::NpmUpgrade)
        );
    }
}
