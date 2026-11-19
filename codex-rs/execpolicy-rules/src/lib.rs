//! Construction, validation, and evaluation of in-memory execution policies.
//!
//! Use `codex-execpolicy` to load rules from a Starlark policy file.

mod decision;
mod error;
mod executable_name;
mod policy;
pub mod rule;

pub use decision::Decision;
pub use error::Error;
pub use error::ErrorLocation;
pub use error::Result;
pub use error::TextPosition;
pub use error::TextRange;
pub use executable_name::executable_lookup_key;
pub use executable_name::executable_path_lookup_key;
pub use policy::Evaluation;
pub use policy::MatchOptions;
pub use policy::Policy;
pub use policy::RequirementsExecPolicy;
pub use rule::NetworkRuleProtocol;
pub use rule::PatternToken;
pub use rule::PrefixPattern;
pub use rule::PrefixRule;
pub use rule::Rule;
pub use rule::RuleMatch;
pub use rule::RuleRef;
