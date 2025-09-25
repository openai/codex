#![allow(clippy::expect_used)]
use std::path::Path;
use tempfile::TempDir;
use wiremock::MockServer;

/// Builder for TestCodexExec. Mirrors TestCodex but tailored to the
/// codex-exec CLI by preparing a temp home and working directory.
pub struct TestCodexExecBuilder {}

impl TestCodexExecBuilder {
    pub fn build(self) -> anyhow::Result<TestCodexExec> {
        let home = TempDir::new()?;
        let cwd = TempDir::new()?;
        Ok(TestCodexExec { home, cwd })
    }
}

pub struct TestCodexExec {
    pub home: TempDir,
    pub cwd: TempDir,
}

impl TestCodexExec {
    /// Returns a pre-configured `assert_cmd::Command` for the `codex-exec`
    /// binary with `CODEX_HOME`, `OPENAI_API_KEY`, and `current_dir` set.
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::cargo_bin("codex-exec")
            .expect("should find binary for codex-exec");
        cmd.current_dir(self.cwd.path())
            .env("CODEX_HOME", self.home.path())
            .env("OPENAI_API_KEY", "dummy");
        cmd
    }

    /// Like `cmd`, but also points `OPENAI_BASE_URL` at the provided mock
    /// server (with the `/v1` suffix) for convenience.
    pub fn cmd_with_server(&self, server: &MockServer) -> assert_cmd::Command {
        let mut cmd = self.cmd();
        let base = format!("{}/v1", server.uri());
        cmd.env("OPENAI_BASE_URL", base);
        cmd
    }

    /// Convenience to override the working directory for a single command
    /// invocation while preserving the same home directory and env setup.
    pub fn cmd_in(&self, dir: &Path) -> assert_cmd::Command {
        let mut cmd = self.cmd();
        cmd.current_dir(dir);
        cmd
    }
}

pub fn test_codex_exec() -> TestCodexExecBuilder {
    TestCodexExecBuilder {}
}
