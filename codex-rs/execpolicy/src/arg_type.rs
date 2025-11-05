#![allow(clippy::needless_lifetimes)]

use crate::error::Error;
use crate::error::Result;
use crate::sed_command::parse_sed_command;
use allocative::Allocative;
use derive_more::derive::Display;
use serde::Serialize;
use starlark::any::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::starlark_value;

#[derive(Debug, Clone, Display, Eq, PartialEq, ProvidesStaticType, Allocative, Serialize)]
#[display("{}", self)]
pub enum ArgType {
    Literal(String),
    /// We cannot say what this argument represents, but it is *not* a file path.
    OpaqueNonFile,
    /// A file (or directory) that can be expected to be read as part of this command.
    ReadableFile,
    /// A file (or directory) that can be expected to be written as part of this command.
    WriteableFile,
    /// Positive integer, like one that is required for `head -n`.
    PositiveInteger,
    /// Bespoke arg type for a safe sed command.
    SedCommand,
    /// Type is unknown: it may or may not be a file.
    Unknown,
}

impl ArgType {
    pub fn validate(&self, value: &str) -> Result<()> {
        match self {
            ArgType::Literal(literal_value) => {
                if value != *literal_value {
                    Err(Error::LiteralValueDidNotMatch {
                        expected: literal_value.clone(),
                        actual: value.to_string(),
                    })
                } else {
                    Ok(())
                }
            }
            ArgType::ReadableFile => {
                if value.is_empty() {
                    Err(Error::EmptyFileName {})
                } else {
                    Ok(())
                }
            }
            ArgType::WriteableFile => {
                if value.is_empty() {
                    Err(Error::EmptyFileName {})
                } else {
                    Ok(())
                }
            }
            ArgType::OpaqueNonFile | ArgType::Unknown => Ok(()),
            ArgType::PositiveInteger => match value.parse::<u64>() {
                Ok(0) => Err(Error::InvalidPositiveInteger {
                    value: value.to_string(),
                }),
                Ok(_) => Ok(()),
                Err(_) => Err(Error::InvalidPositiveInteger {
                    value: value.to_string(),
                }),
            },
            ArgType::SedCommand => parse_sed_command(value),
        }
    }

    pub fn might_write_file(&self) -> bool {
        match self {
            ArgType::WriteableFile | ArgType::Unknown => true,
            ArgType::Literal(_)
            | ArgType::OpaqueNonFile
            | ArgType::PositiveInteger
            | ArgType::ReadableFile
            | ArgType::SedCommand => false,
        }
    }
}

#[starlark_value(type = "ArgType")]
impl<'v> StarlarkValue<'v> for ArgType {
    type Canonical = ArgType;
}
