mod brew;
mod errors;

use async_trait::async_trait;
pub use errors::Error;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;

use crate::brew::BrewInstaller;

const AUTO_UPDATER_STATUS_KEY: &str = "auto_updater.status";

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

#[async_trait]
pub trait Installer: Send + Sync {
    fn version_status(&self) -> Result<UpdateStatus, Error>;

    fn update_available(&self) -> Result<bool, Error> {
        self.version_status().map(|status| status.update_available)
    }

    async fn update(&self) -> Result<String, Error>;
}

pub fn installer() -> Result<Box<dyn Installer>, Error> {
    if let Some(installer) = BrewInstaller::detect()? {
        return Ok(Box::new(installer));
    }
    Err(Error::Unsupported)
}

fn compute_update_status() -> Result<UpdateStatus, Error> {
    installer()?.version_status()
}

pub fn update_status() -> Result<UpdateStatus, Error> {
    compute_update_status()
}

pub fn update_available() -> Result<bool, Error> {
    installer()?.update_available()
}

pub async fn update() -> Result<String, Error> {
    installer()?.update().await
}

pub fn initialize_storage(codex_home: &Path) -> Result<(), Error> {
    codex_internal_storage::initialize(codex_home.to_path_buf());
    Ok(())
}

pub fn read_cached_status() -> Result<Option<UpdateStatus>, Error> {
    match codex_internal_storage::read(AUTO_UPDATER_STATUS_KEY) {
        Ok(Some(value)) => {
            let status =
                serde_json::from_str(&value).map_err(|err| Error::Json(err.to_string()))?;
            Ok(Some(status))
        }
        Ok(None) => Ok(None),
        Err(err) => Err(map_storage_error(err)),
    }
}

pub async fn refresh_status() -> Result<UpdateStatus, Error> {
    let status = compute_update_status()?;
    let serialized = serde_json::to_string(&status).map_err(|err| Error::Json(err.to_string()))?;
    codex_internal_storage::write(AUTO_UPDATER_STATUS_KEY, &serialized)
        .map_err(map_storage_error)?;
    Ok(status)
}

fn map_storage_error(err: codex_internal_storage::InternalStorageError) -> Error {
    match err {
        codex_internal_storage::InternalStorageError::Io(err) => Error::Io(err.to_string()),
        codex_internal_storage::InternalStorageError::Json(err) => Error::Json(err.to_string()),
        codex_internal_storage::InternalStorageError::Uninitialized => {
            Error::Io("internal storage not initialized".into())
        }
    }
}
