use image::ImageFormat;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageProcessingError {
    #[error("failed to read image at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to decode image at {path}: {source}")]
    Decode {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },
    #[error("failed to encode image as {format:?}: {source}")]
    Encode {
        format: ImageFormat,
        #[source]
        source: image::ImageError,
    },
}
