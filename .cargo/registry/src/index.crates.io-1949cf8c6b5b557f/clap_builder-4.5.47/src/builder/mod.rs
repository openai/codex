//! Define [`Command`] line [arguments][`Arg`]

mod action;
mod app_settings;
mod arg;
mod arg_group;
mod arg_predicate;
mod arg_settings;
mod command;
mod ext;
mod os_str;
mod possible_value;
mod range;
mod resettable;
mod str;
mod styled_str;
mod value_hint;
mod value_parser;

#[cfg(debug_assertions)]
mod debug_asserts;

#[cfg(test)]
mod tests;

pub mod styling;

pub use self::str::Str;
pub use action::ArgAction;
pub use arg::Arg;
#[cfg(feature = "unstable-ext")]
pub use arg::ArgExt;
pub use arg_group::ArgGroup;
pub use arg_predicate::ArgPredicate;
pub use command::Command;
#[cfg(feature = "unstable-ext")]
pub use command::CommandExt;
pub use os_str::OsStr;
pub use possible_value::PossibleValue;
pub use range::ValueRange;
pub use resettable::IntoResettable;
pub use resettable::Resettable;
pub use styled_str::StyledStr;
pub use styling::Styles;
pub use value_hint::ValueHint;
pub use value_parser::BoolValueParser;
pub use value_parser::_infer_ValueParser_for;
pub use value_parser::impl_prelude;
pub use value_parser::BoolishValueParser;
pub use value_parser::EnumValueParser;
pub use value_parser::FalseyValueParser;
pub use value_parser::MapValueParser;
pub use value_parser::NonEmptyStringValueParser;
pub use value_parser::OsStringValueParser;
pub use value_parser::PathBufValueParser;
pub use value_parser::PossibleValuesParser;
pub use value_parser::RangedI64ValueParser;
pub use value_parser::RangedU64ValueParser;
pub use value_parser::StringValueParser;
pub use value_parser::TryMapValueParser;
pub use value_parser::TypedValueParser;
pub use value_parser::UnknownArgumentValueParser;
pub use value_parser::ValueParser;
pub use value_parser::ValueParserFactory;
pub use value_parser::_AnonymousValueParser;

#[allow(unused_imports)]
pub(crate) use self::str::Inner as StrInner;
pub(crate) use action::CountType;
pub(crate) use arg_settings::{ArgFlags, ArgSettings};
pub(crate) use command::AppExt;
