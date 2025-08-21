#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#[macro_use]
extern crate starlark;

mod arg_matcher;
mod arg_resolver;
mod arg_type;
mod error;
mod exec_call;
mod execv_checker;
mod opt;
mod policy;
mod policy_parser;
mod program;
mod sed_command;
mod valid_exec;

pub use arg_matcher::ArgMatcher;
pub use arg_resolver::PositionalArg;
pub use arg_type::ArgType;
pub use error::Error;
pub use error::Result;
pub use exec_call::ExecCall;
pub use execv_checker::ExecvChecker;
pub use opt::Opt;
pub use policy::Policy;
pub use policy_parser::PolicyParser;
pub use program::Forbidden;
pub use program::MatchedExec;
pub use program::NegativeExamplePassedCheck;
pub use program::PositiveExampleFailedCheck;
pub use program::ProgramSpec;
pub use sed_command::parse_sed_command;
pub use valid_exec::MatchedArg;
pub use valid_exec::MatchedFlag;
pub use valid_exec::MatchedOpt;
pub use valid_exec::ValidExec;

const DEFAULT_POLICY: &str = include_str!("default.policy");

pub fn get_default_policy() -> starlark::Result<Policy> {
    let parser = PolicyParser::new("#default", DEFAULT_POLICY);
    parser.parse()
}
