mod brew;
mod errors;

use async_trait::async_trait;
pub use errors::Error;

use crate::brew::BrewInstaller;

#[async_trait]
pub trait Installer: Send + Sync {
    fn update_available(&self) -> Result<bool, Error>;

    async fn update(&self) -> Result<String, Error>;
}

pub fn installer() -> Result<Box<dyn Installer>, Error> {
    if let Some(installer) = BrewInstaller::detect()? {
        return Ok(Box::new(installer));
    }
    Err(Error::Unsupported)
}

pub fn update_available() -> Result<bool, Error> {
    installer()?.update_available()
}

pub async fn update() -> Result<String, Error> {
    installer()?.update().await
}
