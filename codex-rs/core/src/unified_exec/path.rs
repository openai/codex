use super::errors::UnifiedExecError;

pub(crate) fn resolve_command_path(command: &str) -> Result<String, UnifiedExecError> {
    if command.is_empty() {
        return Err(UnifiedExecError::MissingCommandLine);
    }

    // Which is the most portable option regarding its current implementation.
    which::which(command)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|_| UnifiedExecError::CommandNotFound {
            command: command.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn returns_error_when_command_is_empty() {
        let error = resolve_command_path("");

        match error {
            Err(UnifiedExecError::MissingCommandLine) => {}
            other => panic!("expected MissingCommandLine error, got {other:?}"),
        }
    }

    #[test]
    fn returns_error_when_command_cannot_be_found() {
        let error = resolve_command_path("this-command-should-not-exist");

        match error {
            Err(UnifiedExecError::CommandNotFound { command }) => {
                assert_eq!(command, "this-command-should-not-exist");
            }
            other => panic!("expected CommandNotFound error, got {other:?}"),
        }
    }

    #[test]
    fn resolves_absolute_command_path() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let command_name = if cfg!(windows) {
            "codex-test-command.bat"
        } else {
            "codex-test-command"
        };
        let command_path = temp_dir.path().join(command_name);

        let command_contents = if cfg!(windows) {
            "@echo off\nexit /b 0\n"
        } else {
            "#!/bin/sh\necho codex\n"
        };

        std::fs::write(&command_path, command_contents).expect("command file should be writeable");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&command_path)
                .expect("metadata should be readable")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&command_path, permissions)
                .expect("permissions should be set");
        }

        let absolute_path = command_path
            .to_str()
            .expect("absolute path should convert to string");

        let resolved_path =
            resolve_command_path(absolute_path).expect("resolver should return an absolute path");

        assert_eq!(resolved_path, absolute_path);
    }
}
