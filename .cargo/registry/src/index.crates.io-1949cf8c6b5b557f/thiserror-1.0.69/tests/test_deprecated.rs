#![deny(deprecated, clippy::all, clippy::pedantic)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[deprecated]
    #[error("...")]
    Deprecated,
}
