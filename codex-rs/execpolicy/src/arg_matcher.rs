#![allow(clippy::needless_lifetimes)]

use allocative::Allocative;
use derive_more::derive::Display;
use starlark::any::ProvidesStaticType;
use starlark::values::string::StarlarkStr;
use starlark::values::{
    AllocValue, Heap, NoSerialize, StarlarkValue, UnpackValue, Value, starlark_value,
};

use crate::arg_type::ArgType;
use crate::starlark::values::ValueLike;

/// Patterns that lists of arguments should be compared against.
#[derive(Clone, Debug, Display, Eq, PartialEq, NoSerialize, ProvidesStaticType, Allocative)]
#[display("{}", self)]
pub enum ArgMatcher {
    /// Literal string value.
    Literal(String),

    /// We cannot say what type of value this should match, but it is *not* a file path.
    OpaqueNonFile,

    /// Required readable file.
    ReadableFile,

    /// Required writeable file.
    WriteableFile,

    /// Non-empty list of readable files.
    ReadableFiles,

    /// Non-empty list of readable files, or empty list, implying readable cwd.
    ReadableFilesOrCwd,

    /// Positive integer, like one that is required for `head -n`.
    PositiveInteger,

    /// Bespoke matcher for safe sed commands.
    SedCommand,

    /// Matches an arbitrary number of arguments without attributing any
    /// particular meaning to them. Caller is responsible for interpreting them.
    UnverifiedVarargs,
}

impl ArgMatcher {
    pub fn cardinality(&self) -> ArgMatcherCardinality {
        match self {
            ArgMatcher::Literal(_)
            | ArgMatcher::OpaqueNonFile
            | ArgMatcher::ReadableFile
            | ArgMatcher::WriteableFile
            | ArgMatcher::PositiveInteger
            | ArgMatcher::SedCommand => ArgMatcherCardinality::One,
            ArgMatcher::ReadableFiles => ArgMatcherCardinality::AtLeastOne,
            ArgMatcher::ReadableFilesOrCwd | ArgMatcher::UnverifiedVarargs => {
                ArgMatcherCardinality::ZeroOrMore
            }
        }
    }

    pub fn arg_type(&self) -> ArgType {
        match self {
            ArgMatcher::Literal(value) => ArgType::Literal(value.clone()),
            ArgMatcher::OpaqueNonFile => ArgType::OpaqueNonFile,
            ArgMatcher::ReadableFile => ArgType::ReadableFile,
            ArgMatcher::WriteableFile => ArgType::WriteableFile,
            ArgMatcher::ReadableFiles => ArgType::ReadableFile,
            ArgMatcher::ReadableFilesOrCwd => ArgType::ReadableFile,
            ArgMatcher::PositiveInteger => ArgType::PositiveInteger,
            ArgMatcher::SedCommand => ArgType::SedCommand,
            ArgMatcher::UnverifiedVarargs => ArgType::Unknown,
        }
    }
}

pub enum ArgMatcherCardinality {
    One,
    AtLeastOne,
    ZeroOrMore,
}

impl ArgMatcherCardinality {
    pub fn is_exact(&self) -> Option<usize> {
        match self {
            ArgMatcherCardinality::One => Some(1),
            ArgMatcherCardinality::AtLeastOne => None,
            ArgMatcherCardinality::ZeroOrMore => None,
        }
    }
}

impl<'v> AllocValue<'v> for ArgMatcher {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        heap.alloc_simple(self)
    }
}

#[starlark_value(type = "ArgMatcher")]
impl<'v> StarlarkValue<'v> for ArgMatcher {
    type Canonical = ArgMatcher;
}

impl<'v> UnpackValue<'v> for ArgMatcher {
    type Error = starlark::Error;

    fn unpack_value_impl(value: Value<'v>) -> starlark::Result<Option<Self>> {
        if let Some(str) = value.downcast_ref::<StarlarkStr>() {
            Ok(Some(ArgMatcher::Literal(str.as_str().to_string())))
        } else {
            Ok(value.downcast_ref::<ArgMatcher>().cloned())
        }
    }
}
