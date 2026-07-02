use std::io;
use std::path::Path;
use std::path::PathBuf;

use super::path_safety::CONFIG_PATH_KEY;
use super::path_safety::git_var_path_from_bytes;
use super::path_safety::invalid_config_source;
use crate::git_command::GitRunner;

pub(super) fn default_system_config_source_candidates(
    git: &GitRunner,
    cwd: &Path,
) -> io::Result<Vec<(&'static str, PathBuf)>> {
    if git_env_bool("GIT_CONFIG_NOSYSTEM")? || std::env::var_os("GIT_CONFIG_SYSTEM").is_some() {
        return Ok(Vec::new());
    }
    // `GIT_CONFIG_SYSTEM` was added to `git var` in Git 2.42. The PSEC-4394
    // boundary treats the selected Git installation and its non-environment
    // compile-time system config as host-owned trusted inputs. For older Git,
    // the exact custom ETC_GITCONFIG path is therefore a documented residual;
    // the derivable prefix/ProgramData paths are still checked separately and
    // the no-includes graph validates every directive the system file exposes.
    let Some(paths) = git_var_config_paths(git, cwd, "GIT_CONFIG_SYSTEM")? else {
        return Ok(Vec::new());
    };
    Ok(paths
        .into_iter()
        .map(|path| ("GIT_CONFIG_SYSTEM", path))
        .collect())
}

pub(super) fn selected_git_home_config_candidates(
    git: &GitRunner,
    cwd: &Path,
) -> io::Result<Vec<PathBuf>> {
    if std::env::var_os("GIT_CONFIG_GLOBAL").is_some() {
        return Ok(Vec::new());
    }
    #[cfg(not(windows))]
    if std::env::var_os("HOME").is_none() {
        return Ok(Vec::new());
    }
    let dot_gitconfig = selected_git_path_candidate(git, cwd, "~/.gitconfig")?;
    let mut candidates = vec![dot_gitconfig.clone()];
    if std::env::var_os("XDG_CONFIG_HOME").is_none_or(|path| path.is_empty()) {
        let home = dot_gitconfig
            .parent()
            .ok_or_else(|| invalid_config_source("selected Git HOME has no parent"))?;
        candidates.push(home.join(".config/git/config"));
    }
    Ok(candidates)
}

pub(super) fn selected_git_prefix_system_candidate(
    git: &GitRunner,
    cwd: &Path,
) -> io::Result<Option<PathBuf>> {
    if git_env_bool("GIT_CONFIG_NOSYSTEM")? || std::env::var_os("GIT_CONFIG_SYSTEM").is_some() {
        return Ok(None);
    }
    selected_git_path_candidate(git, cwd, "%(prefix)/etc/gitconfig").map(Some)
}

fn selected_git_path_candidate(git: &GitRunner, cwd: &Path, raw: &str) -> io::Result<PathBuf> {
    let tempdir = tempfile::tempdir()?;
    let nonexistent_git_dir = tempdir.path().join("nonexistent-git-dir");
    let disabled_config = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut command = git.command_for_cwd(cwd)?;
    command
        .env("GIT_CONFIG_GLOBAL", disabled_config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "0")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .arg("--git-dir")
        .arg(&nonexistent_git_dir)
        .arg("-c")
        .arg(format!("{CONFIG_PATH_KEY}={raw}"))
        .args([
            "config",
            "--null",
            "--no-includes",
            "--path",
            "--get",
            CONFIG_PATH_KEY,
        ]);
    let output = git.output(command)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "isolated selected Git path expansion failed with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let value = output
        .stdout
        .strip_suffix(&[0])
        .ok_or_else(|| invalid_config_source("unterminated selected Git path"))?;
    if value.is_empty() || value.contains(&0) {
        return Err(invalid_config_source("ambiguous selected Git path"));
    }
    git_var_path_from_bytes(value)
}

fn git_var_config_paths(
    git: &GitRunner,
    cwd: &Path,
    variable: &str,
) -> io::Result<Option<Vec<PathBuf>>> {
    let mut command = git.command_for_cwd(cwd)?;
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["var", variable]);
    let output = git.output(command)?;
    parse_git_var_config_paths_result(
        output.status.code(),
        &output.stdout,
        &output.stderr,
        variable,
    )
}

pub(super) fn parse_git_var_config_paths_result(
    status_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
    variable: &str,
) -> io::Result<Option<Vec<PathBuf>>> {
    match status_code {
        Some(0) => parse_git_var_paths(stdout).map(Some),
        Some(1) if stdout.is_empty() && stderr.is_empty() => Ok(Some(Vec::new())),
        // These variables were added in Git 2.42. Older Git reports usage
        // status 129; fall back to its documented environment/home sources.
        Some(129) => Ok(None),
        _ => Err(io::Error::other(format!(
            "git {variable} source probe failed with status {status_code:?}: {}",
            String::from_utf8_lossy(stderr).trim()
        ))),
    }
}

fn parse_git_var_paths(output: &[u8]) -> io::Result<Vec<PathBuf>> {
    output
        .split(|byte| *byte == b'\n')
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = path.strip_suffix(b"\r").unwrap_or(path);
            git_var_path_from_bytes(path)
        })
        .collect()
}

pub(super) fn legacy_primary_config_source_candidates() -> io::Result<Vec<(&'static str, PathBuf)>>
{
    let mut candidates = Vec::new();
    match std::env::var_os("GIT_CONFIG_GLOBAL") {
        Some(path) if !path.is_empty() => {
            candidates.push(("GIT_CONFIG_GLOBAL", PathBuf::from(path)));
        }
        Some(_) => {}
        None => {
            let homes = git_home_directories();
            match std::env::var_os("XDG_CONFIG_HOME") {
                Some(xdg) if !xdg.is_empty() => candidates.push((
                    "XDG_CONFIG_HOME Git config",
                    PathBuf::from(xdg).join("git/config"),
                )),
                _ => {
                    #[cfg(windows)]
                    if let Some(app_data) = std::env::var_os("APPDATA")
                        && !app_data.is_empty()
                    {
                        candidates.push((
                            "APPDATA Git config",
                            PathBuf::from(app_data).join("Git/config"),
                        ));
                    }
                    for home in &homes {
                        candidates.push((
                            "HOME XDG Git config",
                            home_config_path(home, ".config/git/config"),
                        ));
                    }
                }
            }
            for home in &homes {
                candidates.push(("HOME Git config", home_config_path(home, ".gitconfig")));
            }
        }
    }
    if !git_env_bool("GIT_CONFIG_NOSYSTEM")? {
        match std::env::var_os("GIT_CONFIG_SYSTEM") {
            Some(path) if !path.is_empty() => {
                candidates.push(("GIT_CONFIG_SYSTEM", PathBuf::from(path)));
            }
            Some(_) => {}
            None => {
                #[cfg(windows)]
                if let Some(program_data) = std::env::var_os("PROGRAMDATA")
                    && !program_data.is_empty()
                {
                    candidates.push((
                        "PROGRAMDATA Git config",
                        PathBuf::from(program_data).join("Git/config"),
                    ));
                }
            }
        }
    }
    Ok(candidates)
}

fn git_home_directories() -> Vec<std::ffi::OsString> {
    if let Some(home) = std::env::var_os("HOME") {
        return vec![home];
    }
    #[cfg(windows)]
    {
        let mut homes = Vec::new();
        if let (Some(drive), Some(path)) =
            (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
        {
            let mut home = drive;
            home.push(path);
            homes.push(home);
        }
        if let Some(profile) = std::env::var_os("USERPROFILE")
            && !homes.iter().any(|home| *home == profile)
        {
            homes.push(profile);
        }
        return homes;
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn home_config_path(home: &std::ffi::OsStr, suffix: &str) -> PathBuf {
    if home.is_empty() {
        PathBuf::from(std::path::MAIN_SEPARATOR.to_string()).join(suffix)
    } else {
        PathBuf::from(home).join(suffix)
    }
}

fn git_env_bool(name: &str) -> io::Result<bool> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(false);
    };
    let value = value
        .to_str()
        .ok_or_else(|| invalid_config_source("non-UTF-8 Git boolean environment value"))?;
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "" | "0" | "false" | "no" | "off" => Ok(false),
        value => value
            .parse::<i32>()
            .map(|value| value != 0)
            .map_err(|_| invalid_config_source("invalid Git boolean environment value")),
    }
}

#[cfg(windows)]
pub(super) fn is_disabled_primary_config_path(path: &Path) -> bool {
    path.as_os_str()
        .to_str()
        .is_some_and(|path| path.eq_ignore_ascii_case("NUL"))
}

#[cfg(not(windows))]
pub(super) fn is_disabled_primary_config_path(_path: &Path) -> bool {
    false
}
