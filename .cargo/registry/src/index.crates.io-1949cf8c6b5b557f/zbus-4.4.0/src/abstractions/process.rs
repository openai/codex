use std::{ffi::OsStr, io::Error, process::Output};

/// An asynchronous wrapper around running and getting command output
pub async fn run<I, S>(program: S, args: I) -> Result<Output, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    #[cfg(not(feature = "tokio"))]
    return async_process::Command::new(program)
        .args(args)
        .output()
        .await;

    #[cfg(feature = "tokio")]
    return tokio::process::Command::new(program)
        .args(args)
        .output()
        .await;
}
