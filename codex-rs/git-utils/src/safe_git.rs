use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::process::Command;

pub(crate) const DISABLED_HOOKS_PATH: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
pub(crate) const EXECUTABLE_GIT_CONFIG_PATTERN: &str =
    r"^(filter\..*\.(clean|smudge|process|required)|merge\..*\.driver)$";
pub(crate) type GitConfigOverride = (String, String);

pub(crate) fn configured_executable_git_config_overrides(
    cwd: &Path,
) -> io::Result<Vec<GitConfigOverride>> {
    let output = Command::new("git")
        .args([
            "config",
            "--null",
            "--name-only",
            "--get-regexp",
            EXECUTABLE_GIT_CONFIG_PATTERN,
        ])
        .current_dir(cwd)
        .output()?;
    if output
        .status
        .code()
        .is_some_and(|code| code == 0 || code == 1)
    {
        return Ok(executable_git_config_overrides_from_output(&output.stdout));
    }

    Err(io::Error::other(format!(
        "git config probe failed with status {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

pub(crate) fn executable_git_config_overrides_from_output(stdout: &[u8]) -> Vec<GitConfigOverride> {
    let mut filter_drivers = BTreeSet::new();
    let mut merge_drivers = BTreeSet::new();

    for key in stdout
        .split(|byte| *byte == 0)
        .filter(|key| !key.is_empty())
        .filter_map(|key| std::str::from_utf8(key).ok())
    {
        if let Some(driver) = key
            .strip_suffix(".clean")
            .or_else(|| key.strip_suffix(".smudge"))
            .or_else(|| key.strip_suffix(".process"))
            .or_else(|| key.strip_suffix(".required"))
        {
            filter_drivers.insert(driver.to_string());
        } else if key.starts_with("merge.") && key.ends_with(".driver") {
            merge_drivers.insert(key.to_string());
        }
    }

    filter_drivers
        .into_iter()
        .flat_map(|driver| {
            [
                (format!("{driver}.clean"), String::new()),
                (format!("{driver}.smudge"), String::new()),
                (format!("{driver}.process"), String::new()),
                (format!("{driver}.required"), "false".to_string()),
            ]
        })
        .chain(
            merge_drivers
                .into_iter()
                .map(|driver| (driver, String::new())),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn executable_git_config_overrides_clear_filters_and_merge_drivers() {
        let output = b"filter.x=y.clean\0filter.x=y.required\0merge.pwn.driver\0";

        assert_eq!(
            executable_git_config_overrides_from_output(output),
            vec![
                ("filter.x=y.clean".to_string(), String::new()),
                ("filter.x=y.smudge".to_string(), String::new()),
                ("filter.x=y.process".to_string(), String::new()),
                ("filter.x=y.required".to_string(), "false".to_string()),
                ("merge.pwn.driver".to_string(), String::new()),
            ]
        );
    }
}
