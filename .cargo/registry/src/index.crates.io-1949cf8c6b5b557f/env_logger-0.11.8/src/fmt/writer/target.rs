/// Log target, either `stdout`, `stderr` or a custom pipe.
#[non_exhaustive]
#[derive(Default)]
pub enum Target {
    /// Logs will be sent to standard output.
    Stdout,
    /// Logs will be sent to standard error.
    #[default]
    Stderr,
    /// Logs will be sent to a custom pipe.
    Pipe(Box<dyn std::io::Write + Send + 'static>),
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Stdout => "stdout",
                Self::Stderr => "stderr",
                Self::Pipe(_) => "pipe",
            }
        )
    }
}
