use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;

use crate::exit_status::handle_exit_status;

const RELEASE_URL: &str = "https://github.com/openai/codex/releases/latest";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallMethod {
    Npm,
    Brew,
}

#[derive(Debug)]
struct InstallEnvironment {
    managed_by_npm: bool,
    current_exe: Option<PathBuf>,
    is_macos: bool,
}

impl InstallEnvironment {
    fn from_system() -> Self {
        Self {
            managed_by_npm: std::env::var_os("CODEX_MANAGED_BY_NPM").is_some(),
            current_exe: std::env::current_exe().ok(),
            is_macos: cfg!(target_os = "macos"),
        }
    }
}

pub async fn run_update_command() -> ! {
    let env = InstallEnvironment::from_system();
    let Some(method) = detect_install_method(&env) else {
        eprintln!("Unable to determine how Codex was installed.");
        eprintln!("If you installed Codex with npm, run `npm install -g @openai/codex@latest`.",);
        eprintln!("If you installed Codex with Homebrew, run `brew upgrade codex`.");
        eprintln!("For other installation methods, see {RELEASE_URL}.");
        std::process::exit(1);
    };

    let (program, args) = match method {
        InstallMethod::Npm => ("npm", ["install", "-g", "@openai/codex@latest"]),
        InstallMethod::Brew => ("brew", ["upgrade", "codex"]),
    };

    run_external_command(program, &args).await;
}

fn detect_install_method(env: &InstallEnvironment) -> Option<InstallMethod> {
    if env.managed_by_npm {
        return Some(InstallMethod::Npm);
    }

    if env.is_macos
        && env
            .current_exe
            .as_deref()
            .is_some_and(is_homebrew_executable)
    {
        return Some(InstallMethod::Brew);
    }

    None
}

fn is_homebrew_executable(exe: &Path) -> bool {
    const HOMEBREW_PREFIXES: &[&str] = &["/opt/homebrew", "/usr/local"];
    HOMEBREW_PREFIXES
        .iter()
        .any(|prefix| exe.starts_with(prefix))
}

async fn run_external_command(program: &str, args: &[&str]) -> ! {
    let command_display = format_command(program, args);
    eprintln!("Running `{command_display}` to update Codex...");

    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await;

    match status {
        Ok(status) => handle_exit_status(status),
        Err(err) => {
            eprintln!("Failed to execute `{command_display}`: {err}");
            std::process::exit(1);
        }
    }
}

fn format_command(program: &str, args: &[&str]) -> String {
    let mut command = String::from(program);
    for arg in args {
        command.push(' ');
        command.push_str(arg);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn detects_npm_when_env_var_is_present() {
        let env = InstallEnvironment {
            managed_by_npm: true,
            current_exe: Some(PathBuf::from("/opt/homebrew/bin/codex")),
            is_macos: true,
        };
        assert_eq!(detect_install_method(&env), Some(InstallMethod::Npm));
    }

    #[test]
    fn detects_homebrew_install_on_macos() {
        let env = InstallEnvironment {
            managed_by_npm: false,
            current_exe: Some(PathBuf::from("/opt/homebrew/bin/codex")),
            is_macos: true,
        };
        assert_eq!(detect_install_method(&env), Some(InstallMethod::Brew));
    }

    #[test]
    fn returns_none_when_install_method_is_unknown() {
        let env = InstallEnvironment {
            managed_by_npm: false,
            current_exe: Some(PathBuf::from("/tmp/codex")),
            is_macos: false,
        };
        assert_eq!(detect_install_method(&env), None);
    }

    #[test]
    fn homebrew_prefixes_are_detected() {
        assert!(is_homebrew_executable(Path::new("/opt/homebrew/bin/codex")));
        assert!(is_homebrew_executable(Path::new("/usr/local/bin/codex")));
        assert!(!is_homebrew_executable(Path::new(
            "/home/user/.local/bin/codex"
        )));
    }

    #[test]
    fn command_formatting_is_readable() {
        assert_eq!(
            format_command("npm", &["install", "-g", "@openai/codex@latest"]),
            "npm install -g @openai/codex@latest"
        );
    }
}
