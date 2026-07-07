use codex_shell_command::parse_command::extract_shell_command;
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    pub fn command_name(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Bun => "bun",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DependencyInstallCommand {
    pub package_manager: PackageManager,
}

pub fn detect_dependency_install_command(command: &[String]) -> Option<DependencyInstallCommand> {
    let tokens = if let Some((_, script)) = extract_shell_command(command) {
        shlex::split(script)?
    } else {
        command.to_vec()
    };
    if tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "&&" | "||" | ";" | "|" | "&" | ">" | ">>" | "<"
        )
    }) {
        return None;
    }

    let executable = Path::new(tokens.first()?).file_name()?.to_str()?;
    let package_manager = match executable {
        "npm" => PackageManager::Npm,
        "pnpm" => PackageManager::Pnpm,
        "yarn" => PackageManager::Yarn,
        "bun" => PackageManager::Bun,
        _ => return None,
    };
    let subcommand = tokens.get(1)?.as_str();
    let is_add = match package_manager {
        PackageManager::Npm => {
            matches!(subcommand, "install" | "i" | "add") && has_dependency_argument(&tokens[2..])
        }
        PackageManager::Pnpm => {
            subcommand == "add"
                || (subcommand == "install" && has_dependency_argument(&tokens[2..]))
        }
        PackageManager::Yarn | PackageManager::Bun => subcommand == "add",
    };

    is_add.then_some(DependencyInstallCommand { package_manager })
}

pub fn is_dependency_manifest_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "package.json"
                    | "package-lock.json"
                    | "npm-shrinkwrap.json"
                    | "pnpm-lock.yaml"
                    | "yarn.lock"
                    | "bun.lock"
                    | "bun.lockb"
            )
        })
}

fn has_dependency_argument(arguments: &[String]) -> bool {
    arguments.iter().any(|argument| {
        !argument.starts_with('-') && !matches!(argument.as_str(), "install" | "add")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn recognizes_common_dependency_add_commands() {
        for (command, package_manager) in [
            (
                strings(&["npm", "install", "zod@3.23.8"]),
                PackageManager::Npm,
            ),
            (
                strings(&["pnpm", "add", "zod@3.23.8"]),
                PackageManager::Pnpm,
            ),
            (
                strings(&["yarn", "add", "zod@3.23.8"]),
                PackageManager::Yarn,
            ),
            (strings(&["bun", "add", "zod@3.23.8"]), PackageManager::Bun),
            (
                strings(&["/bin/zsh", "-lc", "npm install --save-dev zod@3.23.8"]),
                PackageManager::Npm,
            ),
        ] {
            assert_eq!(
                detect_dependency_install_command(&command),
                Some(DependencyInstallCommand { package_manager })
            );
        }
    }

    #[test]
    fn ignores_plain_installs_and_shell_indirection() {
        assert_eq!(
            detect_dependency_install_command(&strings(&["npm", "install"])),
            None
        );
        assert_eq!(
            detect_dependency_install_command(&strings(&[
                "/bin/zsh",
                "-lc",
                "npm install zod@3.23.8 && echo done"
            ])),
            None
        );
    }

    #[test]
    fn recognizes_dependency_manifest_paths() {
        assert!(is_dependency_manifest_path(Path::new("web/package.json")));
        assert!(is_dependency_manifest_path(Path::new("package-lock.json")));
        assert!(!is_dependency_manifest_path(Path::new("Cargo.toml")));
    }
}
