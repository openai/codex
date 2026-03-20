use thiserror::Error;

use crate::model_provider_info::EnvKeyError;

pub(crate) type Result<T> = std::result::Result<T, ModelsError>;

#[derive(Debug, Error)]
pub(crate) enum ModelsError {
    #[error(transparent)]
    EnvVar(#[from] EnvKeyError),
    #[error("failed to read auth token: {0}")]
    Auth(#[from] std::io::Error),
    #[error("timed out while refreshing remote models")]
    Timeout,
    #[error("failed to refresh remote models: {0}")]
    Api(String),
}
