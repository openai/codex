#[cfg(any(not(debug_assertions), test))]
use codex_install_context::InstallContext;
use codex_install_context::InstallManager;

pub type UpdateAction = InstallManager;

/// Returns the list of command-line arguments for invoking the update.
pub fn command_args(action: UpdateAction) -> (String, Vec<String>) {
    match action {
        InstallManager::Npm => (
            "npm".to_string(),
            vec!["install".into(), "-g".into(), "@openai/codex".into()],
        ),
        InstallManager::Bun => (
            "bun".to_string(),
            vec!["install".into(), "-g".into(), "@openai/codex".into()],
        ),
        InstallManager::Native => {
            #[cfg(windows)]
            {
                (
                    "powershell".to_string(),
                    vec![
                        "-NoProfile".into(),
                        "-ExecutionPolicy".into(),
                        "Bypass".into(),
                        "-Command".into(),
                        "$tmp = New-TemporaryFile; Invoke-WebRequest -Uri 'https://chatgpt.com/codex/install.ps1' -OutFile $tmp; & $tmp; Remove-Item $tmp".into(),
                    ],
                )
            }
            #[cfg(not(windows))]
            {
                (
                    "sh".to_string(),
                    vec![
                        "-c".into(),
                        "tmp=\"$(mktemp)\" && if command -v curl >/dev/null 2>&1; then curl -fsSL 'https://chatgpt.com/codex/install.sh' -o \"$tmp\"; elif command -v wget >/dev/null 2>&1; then wget -q -O \"$tmp\" 'https://chatgpt.com/codex/install.sh'; else echo 'curl or wget is required to update Codex.' >&2; rm -f \"$tmp\"; exit 1; fi && sh \"$tmp\"; status=$?; rm -f \"$tmp\"; exit $status".into(),
                    ],
                )
            }
        }
        InstallManager::Brew => (
            "brew".to_string(),
            vec!["upgrade".into(), "--cask".into(), "codex".into()],
        ),
        InstallManager::Other => {
            unreachable!("non-updatable installs should not reach command_args")
        }
    }
}

/// Returns string representation of the command-line arguments for invoking the update.
pub fn command_str(action: UpdateAction) -> String {
    let (command, args) = command_args(action);
    shlex::try_join(std::iter::once(command.as_str()).chain(args.iter().map(String::as_str)))
        .unwrap_or_else(|_| format!("{command} {}", args.join(" ")))
}

#[cfg(not(debug_assertions))]
pub(crate) fn get_update_action() -> Option<UpdateAction> {
    update_action_for_context(InstallContext::current())
}

#[cfg(any(not(debug_assertions), test))]
fn detect_update_action(
    is_macos: bool,
    current_exe: &std::path::Path,
    managed_by_npm: bool,
    managed_by_bun: bool,
) -> Option<UpdateAction> {
    let context =
        InstallContext::from_exe(is_macos, Some(current_exe), managed_by_npm, managed_by_bun);
    update_action_for_context(&context)
}

#[cfg(any(not(debug_assertions), test))]
fn update_action_for_context(context: &InstallContext) -> Option<UpdateAction> {
    match context.manager() {
        InstallManager::Npm => Some(InstallManager::Npm),
        InstallManager::Bun => Some(InstallManager::Bun),
        InstallManager::Native => Some(InstallManager::Native),
        InstallManager::Brew => Some(InstallManager::Brew),
        InstallManager::Other => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;

    #[test]
    fn detects_update_action_without_env_mutation() {
        assert_eq!(
            detect_update_action(
                /*is_macos*/ false,
                std::path::Path::new("/any/path"),
                /*managed_by_npm*/ false,
                /*managed_by_bun*/ false
            ),
            None
        );
        assert_eq!(
            detect_update_action(false, std::path::Path::new("/any/path"), true, false),
            Some(InstallManager::Npm)
        );
        assert_eq!(
            detect_update_action(false, std::path::Path::new("/any/path"), false, true),
            Some(InstallManager::Bun)
        );
        assert_eq!(
            detect_update_action(
                /*is_macos*/ true,
                std::path::Path::new("/opt/homebrew/bin/codex"),
                /*managed_by_npm*/ false,
                /*managed_by_bun*/ false
            ),
            Some(InstallManager::Brew)
        );
        assert_eq!(
            detect_update_action(
                /*is_macos*/ true,
                std::path::Path::new("/usr/local/bin/codex"),
                /*managed_by_npm*/ false,
                /*managed_by_bun*/ false
            ),
            Some(InstallManager::Brew)
        );
    }

    #[test]
    fn detects_native_update_action_from_metadata() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let release_dir = root.path().join("1.2.3-x86_64-unknown-linux-musl");
        fs::create_dir(&release_dir)?;
        fs::write(
            release_dir.join("metadata.toml"),
            "install_method = \"native\"\nversion = \"1.2.3\"\ntarget = \"x86_64-unknown-linux-musl\"\n",
        )?;
        let exe_path = release_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        fs::write(&exe_path, "")?;

        assert_eq!(
            detect_update_action(false, &exe_path, false, false),
            Some(InstallManager::Native)
        );
        Ok(())
    }
}
