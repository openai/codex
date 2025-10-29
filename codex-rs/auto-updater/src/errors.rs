use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unsupported install method")]
    Unsupported,
    #[error("brew not found in PATH")]
    BrewMissing,
    #[error("command failed: {command}")]
    Command {
        command: String,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    #[error("json parse error: {0}")]
    Json(String),
    #[error("version parse error: {0}")]
    Version(String),
    #[error("io error: {0}")]
    Io(String),
}
