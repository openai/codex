use super::Status;

#[derive(Debug)]
pub enum SocksV5Error {
    HostTooLong,
    Auth(AuthError),
    Command(Status),
}

#[derive(Debug)]
pub enum AuthError {
    Unsupported,
    MethodMismatch,
    Failed,
}

impl From<Status> for SocksV5Error {
    fn from(err: Status) -> Self {
        Self::Command(err)
    }
}

impl From<AuthError> for SocksV5Error {
    fn from(err: AuthError) -> Self {
        Self::Auth(err)
    }
}

impl std::fmt::Display for SocksV5Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostTooLong => f.write_str("host address is more than 255 characters"),
            Self::Command(e) => e.fmt(f),
            Self::Auth(e) => e.fmt(f),
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "server does not support user/pass authentication",
            Self::MethodMismatch => "server implements authentication incorrectly",
            Self::Failed => "credentials not accepted",
        })
    }
}
