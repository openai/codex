mod brew;
mod errors;

use async_trait::async_trait;
pub use errors::Error;

use crate::brew::BrewInstaller;

#[derive(Debug)]
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

pub fn update_status() -> Result<UpdateStatus, Error> {
    installer()?.version_status()
}

pub fn update_available() -> Result<bool, Error> {
    installer()?.update_available()
}

pub async fn update() -> Result<String, Error> {
    installer()?.update().await
}
