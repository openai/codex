use crate::process::WindowsProcessLaunch;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::path::Prefix;

const DEFAULT_PATHEXT: &str = ".COM;.EXE;.BAT;.CMD";

/// Resolves the first command argument using only the child environment and requested cwd.
pub fn resolve_windows_executable(
    command: &[String],
    cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Result<PathBuf> {
    if command.iter().any(|arg| arg.contains('\0')) {
        return Err(anyhow!("Windows command arguments may not contain NUL"));
    }
    let program = command
        .first()
        .ok_or_else(|| anyhow!("cannot resolve an empty Windows command"))?;
    if program.is_empty() {
        return Err(anyhow!("cannot resolve an empty Windows executable"));
    }
    if !cwd.is_absolute() {
        return Err(anyhow!("Windows executable cwd must be absolute"));
    }

    let program_path = Path::new(program);
    if is_drive_relative(program_path) || (program_path.has_root() && !program_path.is_absolute()) {
        return Err(anyhow!(
            "drive-relative and root-relative Windows executable paths are not supported"
        ));
    }

    let has_path = program_path.is_absolute()
        || program.contains(['\\', '/'])
        || program_path.components().count() > 1;
    let bases = if has_path {
        vec![if program_path.is_absolute() {
            program_path.to_path_buf()
        } else {
            cwd.join(program_path)
        }]
    } else {
        windows_search_dirs(cwd, env_map)
            .into_iter()
            .map(|dir| dir.join(program_path))
            .collect()
    };
    let extensions = windows_env_value(env_map, "PATHEXT")
        .unwrap_or(DEFAULT_PATHEXT)
        .split(';')
        .map(str::trim)
        .filter(|extension| extension.starts_with('.') && extension.len() > 1)
        .collect::<Vec<_>>();

    for base in bases {
        if !base.is_absolute() {
            continue;
        }
        if program_path.extension().is_some() {
            if is_existing_file(&base)? {
                return ensure_directly_launchable(base);
            }
            continue;
        }
        for extension in &extensions {
            let mut candidate = base.clone().into_os_string();
            candidate.push(extension);
            let candidate = PathBuf::from(candidate);
            if is_existing_file(&candidate)? {
                return ensure_directly_launchable(candidate);
            }
        }
        if is_existing_file(&base)? {
            return ensure_directly_launchable(base);
        }
    }

    Err(anyhow!(
        "Windows executable `{program}` was not found using the child PATH and PATHEXT"
    ))
}

pub(crate) fn resolve_windows_launch(
    mut launch: WindowsProcessLaunch,
    cwd: &Path,
    env_map: &HashMap<String, String>,
) -> Result<WindowsProcessLaunch> {
    if launch.application_path.is_none() {
        launch.application_path = Some(resolve_windows_executable(&launch.command, cwd, env_map)?);
    }
    Ok(launch)
}

fn windows_search_dirs(cwd: &Path, env_map: &HashMap<String, String>) -> Vec<PathBuf> {
    std::iter::once(cwd.to_path_buf())
        .chain(
            windows_env_value(env_map, "PATH")
                .into_iter()
                .flat_map(std::env::split_paths)
                .filter(|dir| {
                    !dir.as_os_str().is_empty()
                        && !is_drive_relative(dir)
                        && !(dir.has_root() && !dir.is_absolute())
                })
                .map(|dir| {
                    if dir.is_absolute() {
                        dir
                    } else {
                        cwd.join(dir)
                    }
                }),
        )
        .collect()
}

fn is_existing_file(path: &Path) -> Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(!metadata.is_dir()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::symlink_metadata(path) {
                Ok(metadata) => Ok(!metadata.is_dir()),
                Err(link_err) if link_err.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(link_err) => Err(link_err)
                    .with_context(|| format!("inspect Windows executable {}", path.display())),
            }
        }
        Err(err) => {
            Err(err).with_context(|| format!("inspect Windows executable {}", path.display()))
        }
    }
}

fn ensure_directly_launchable(path: PathBuf) -> Result<PathBuf> {
    if path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("bat") || extension.eq_ignore_ascii_case("cmd")
        })
    {
        return Err(anyhow!(
            "Windows batch file `{}` must be launched through cmd.exe",
            path.display()
        ));
    }
    Ok(path)
}

fn is_drive_relative(path: &Path) -> bool {
    !path.has_root()
        && matches!(
            path.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_))
        )
}

fn windows_env_value<'a>(env_map: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    env_map
        .get(key)
        .or_else(|| {
            env_map
                .iter()
                .find(|(existing, _)| existing.eq_ignore_ascii_case(key))
                .map(|(_, value)| value)
        })
        .map(String::as_str)
}

#[cfg(test)]
#[path = "command_resolution_tests.rs"]
mod tests;
