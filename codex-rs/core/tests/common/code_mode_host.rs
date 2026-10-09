//! Records launches of the real code-mode host through a Unix wrapper. Each log
//! line represents one process spawn and its arguments, independently of sessions.

use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use tempfile::TempDir;

/// Observes host process launches without replacing either transport implementation.
pub struct CodeModeHostRecorder {
    _directory: TempDir,
    program: PathBuf,
    invocations: PathBuf,
    pid: PathBuf,
}

impl CodeModeHostRecorder {
    pub fn new() -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let program = directory.path().join("code mode host wrapper");
        let invocations = directory.path().join("host invocations");
        let pid = directory.path().join("host pid");
        fs::write(&invocations, "")?;
        let host_program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
        let quoted_host =
            shlex::try_quote(host_program.to_str().context("host path is not UTF-8")?)?;
        let quoted_log = shlex::try_quote(invocations.to_str().context("log path is not UTF-8")?)?;
        let quoted_pid = shlex::try_quote(pid.to_str().context("PID path is not UTF-8")?)?;
        codex_utils_cargo_bin::write_executable(
            &program,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {quoted_log}\nprintf '%s\\n' \"$$\" > {quoted_pid}\nexec {quoted_host} \"$@\"\n"
            ),
        )?;
        Ok(Self {
            _directory: directory,
            program,
            invocations,
            pid,
        })
    }

    pub fn program(&self) -> PathBuf {
        self.program.clone()
    }

    pub fn invocations(&self) -> Result<Vec<String>> {
        Ok(fs::read_to_string(&self.invocations)?
            .lines()
            .map(str::to_string)
            .collect())
    }

    pub fn pid(&self) -> Result<u32> {
        Ok(fs::read_to_string(&self.pid)?.trim().parse()?)
    }
}
