//! Profile seeding shared by shell snapshot capture and other initialized shells.
//! Preserve the existing scripts; callers own the shell's launch flags and lifecycle.

use crate::shell_detect::ShellType;

/// Load the interactive configuration used by snapshot capture in a login shell.
/// The caller must already launch the shell with its usual login startup flags.
/// Only Bash and Zsh are supported here; POSIX sh's ENV handling remains in capture.
pub fn shell_startup_script(shell_type: ShellType) -> &'static str {
    match shell_type {
        ShellType::Zsh => {
            r#"if [[ -n "${ZDOTDIR-}" ]]; then
  rc="$ZDOTDIR/.zshrc"
elif [[ -n "${HOME-}" ]]; then
  rc="$HOME/.zshrc"
else
  rc=
fi
[[ -r "$rc" ]] && . "$rc"
"#
        }
        ShellType::Bash => {
            r#"if [ -z "${BASH_ENV-}" ] && [ -n "${HOME-}" ] && [ -r "$HOME/.bashrc" ]; then
  . "$HOME/.bashrc"
fi
"#
        }
        ShellType::Sh | ShellType::PowerShell | ShellType::Cmd => "",
    }
}
