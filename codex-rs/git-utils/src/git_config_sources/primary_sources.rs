use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use super::path_safety::CONFIG_PATH_KEY;
use super::path_safety::git_var_path_from_bytes;
use super::path_safety::invalid_config_source;
use crate::git_command::GitRunner;
use crate::git_config::parse_git_boolean_symmetric_i32;

pub(super) fn selected_git_home_config_candidates(
    git: &GitRunner,
    cwd: &Path,
) -> io::Result<Vec<PathBuf>> {
    if git.config_environment_value("GIT_CONFIG_GLOBAL").is_some() {
        return Ok(Vec::new());
    }
    #[cfg(not(windows))]
    if git.config_environment_value("HOME").is_none() {
        return Ok(Vec::new());
    }
    let dot_gitconfig = selected_git_path_candidate(git, cwd, "~/.gitconfig")?;
    let mut candidates = vec![dot_gitconfig.clone()];
    if git
        .config_environment_value("XDG_CONFIG_HOME")
        .is_none_or(OsStr::is_empty)
    {
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
    if git_env_bool(git, "GIT_CONFIG_NOSYSTEM")?
        || git.config_environment_value("GIT_CONFIG_SYSTEM").is_some()
    {
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

pub(super) fn legacy_primary_config_source_candidates(
    git: &GitRunner,
) -> io::Result<Vec<(&'static str, PathBuf)>> {
    let mut candidates = Vec::new();
    match git.config_environment_value("GIT_CONFIG_GLOBAL") {
        Some(path) if !path.is_empty() => {
            candidates.push(("GIT_CONFIG_GLOBAL", PathBuf::from(path)));
        }
        Some(_) => {}
        None => {
            let homes = git_home_directories(git);
            match git.config_environment_value("XDG_CONFIG_HOME") {
                Some(xdg) if !xdg.is_empty() => candidates.push((
                    "XDG_CONFIG_HOME Git config",
                    PathBuf::from(xdg).join("git/config"),
                )),
                _ => {
                    #[cfg(windows)]
                    if let Some(app_data) = git.config_environment_value("APPDATA")
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
    if !git_env_bool(git, "GIT_CONFIG_NOSYSTEM")? {
        match git.config_environment_value("GIT_CONFIG_SYSTEM") {
            Some(path) if !path.is_empty() => {
                candidates.push(("GIT_CONFIG_SYSTEM", PathBuf::from(path)));
            }
            Some(_) => {}
            None => {
                #[cfg(windows)]
                if let Some(program_data) = git.config_environment_value("PROGRAMDATA")
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

fn git_home_directories(git: &GitRunner) -> Vec<std::ffi::OsString> {
    if let Some(home) = git.config_environment_value("HOME") {
        return vec![home.to_owned()];
    }
    #[cfg(windows)]
    {
        let mut homes = Vec::new();
        if let (Some(drive), Some(path)) = (
            git.config_environment_value("HOMEDRIVE"),
            git.config_environment_value("HOMEPATH"),
        ) {
            let mut home = drive.to_owned();
            home.push(path);
            homes.push(home);
        }
        if let Some(profile) = git.config_environment_value("USERPROFILE")
            && !homes.iter().any(|home| home == profile)
        {
            homes.push(profile.to_owned());
        }
        homes
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

fn git_env_bool(git: &GitRunner, name: &str) -> io::Result<bool> {
    let Some(value) = git.config_environment_value(name) else {
        return Ok(false);
    };
    let value = value
        .to_str()
        .ok_or_else(|| invalid_config_source("non-UTF-8 Git boolean environment value"))?;
    parse_git_boolean_symmetric_i32(value.as_bytes())
        .ok_or_else(|| invalid_config_source("invalid Git boolean environment value"))
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
