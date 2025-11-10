pub mod command;
pub mod decision;
pub mod error;
pub mod parser;
pub mod policy;
pub mod rule;

pub use command::tokenize_command;
pub use decision::Decision;
pub use error::Error;
pub use error::Result;
pub use parser::PolicyParser;
pub use policy::Evaluation;
pub use policy::Policy;
pub use rule::Rule;
pub use rule::RuleMatch;

/// Load the default bundled policy.
pub fn load_default_policy() -> Result<Policy> {
    let policy_src = include_str!("default.policy");
    let parser = PolicyParser::new("default.policy", policy_src);
    parser.parse()
}
