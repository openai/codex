/// An error which may occur when exporting a type
#[derive(thiserror::Error, Debug)]
pub enum ExportError {
    #[error("this type cannot be exported")]
    CannotBeExported(&'static str),
    #[cfg(feature = "format")]
    #[error("an error occurred while formatting the generated typescript output")]
    Formatting(String),
    #[error("an error occurred while performing IO")]
    Io(#[from] std::io::Error),
    #[error("the environment variable CARGO_MANIFEST_DIR is not set")]
    ManifestDirNotSet,
    #[error("an error occurred while writing to a formatted buffer")]
    Fmt(#[from] std::fmt::Error),
}
