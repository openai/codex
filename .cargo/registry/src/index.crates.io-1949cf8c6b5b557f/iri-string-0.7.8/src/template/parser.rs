//! URI Template parser.

pub(super) mod char;
pub(super) mod validate;

pub(super) use self::validate::validate_template_str;
