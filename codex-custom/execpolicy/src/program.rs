use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;

use crate::ArgType;
use crate::ExecCall;
use crate::arg_matcher::ArgMatcher;
use crate::arg_resolver::PositionalArg;
use crate::arg_resolver::resolve_observed_args_with_patterns;
use crate::error::Error;
use crate::error::Result;
use crate::opt::Opt;
use crate::opt::OptMeta;
use crate::valid_exec::MatchedFlag;
use crate::valid_exec::MatchedOpt;
use crate::valid_exec::ValidExec;

#[derive(Debug)]
pub struct ProgramSpec {
    pub program: String,
    pub system_path: Vec<String>,
    pub option_bundling: bool,
    pub combined_format: bool,
    pub allowed_options: HashMap<String, Opt>,
    pub arg_patterns: Vec<ArgMatcher>,
    forbidden: Option<String>,
    required_options: HashSet<String>,
    should_match: Vec<Vec<String>>,
    should_not_match: Vec<Vec<String>>,
}

impl ProgramSpec {
    pub fn new(
        program: String,
        system_path: Vec<String>,
        option_bundling: bool,
        combined_format: bool,
        allowed_options: HashMap<String, Opt>,
        arg_patterns: Vec<ArgMatcher>,
        forbidden: Option<String>,
        should_match: Vec<Vec<String>>,
        should_not_match: Vec<Vec<String>>,
    ) -> Self {
        let required_options = allowed_options
            .iter()
            .filter_map(|(name, opt)| {
                if opt.required {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();
        Self {
            program,
            system_path,
            option_bundling,
            combined_format,
            allowed_options,
            arg_patterns,
            forbidden,
            required_options,
            should_match,
            should_not_match,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum MatchedExec {
    Match { exec: ValidExec },
    Forbidden { cause: Forbidden, reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum Forbidden {
    Program {
        program: String,
        exec_call: ExecCall,
    },
    Arg {
        arg: String,
        exec_call: ExecCall,
    },
    Exec {
        exec: ValidExec,
    },
}

impl ProgramSpec {
    // TODO(mbolin): The idea is that there should be a set of rules defined for
    // a program and the args should be checked against the rules to determine
    // if the program should be allowed to run.
    pub fn check(&self, exec_call: &ExecCall) -> Result<MatchedExec> {
        let mut expecting_option_value: Option<(String, ArgType)> = None;
        let mut args = Vec::<PositionalArg>::new();
        let mut matched_flags = Vec::<MatchedFlag>::new();
        let mut matched_opts = Vec::<MatchedOpt>::new();

        for (index, arg) in exec_call.args.iter().enumerate() {
            if let Some(expected) = expecting_option_value {
                // If we are expecting an option value, then the next argument
                // should be the value for the option.
                // This had better not be another option!
                let (name, arg_type) = expected;
                if arg.starts_with("-") {
                    return Err(Error::OptionFollowedByOptionInsteadOfValue {
                        program: self.program.clone(),
                        option: name,
                        value: arg.clone(),
                    });
                }

                matched_opts.push(MatchedOpt::new(&name, arg, arg_type)?);
                expecting_option_value = None;
            } else if arg == "--" {
                return Err(Error::DoubleDashNotSupportedYet {
                    program: self.program.clone(),
                });
            } else if arg.starts_with("-") {
                match self.allowed_options.get(arg) {
                    Some(opt) => {
                        match &opt.meta {
                            OptMeta::Flag => {
                                matched_flags.push(MatchedFlag { name: arg.clone() });
                                // A flag does not expect an argument: continue.
                                continue;
                            }
                            OptMeta::Value(arg_type) => {
                                expecting_option_value = Some((arg.clone(), arg_type.clone()));
                                continue;
                            }
                        }
                    }
                    None => {
                        // It could be an --option=value style flag...
                    }
                }

                return Err(Error::UnknownOption {
                    program: self.program.clone(),
                    option: arg.clone(),
                });
            } else {
                args.push(PositionalArg {
                    index,
                    value: arg.clone(),
                });
            }
        }

        if let Some(expected) = expecting_option_value {
            let (name, _arg_type) = expected;
            return Err(Error::OptionMissingValue {
                program: self.program.clone(),
                option: name,
            });
        }

        let matched_args =
            resolve_observed_args_with_patterns(&self.program, args, &self.arg_patterns)?;

        // Verify all required options are present.
        let matched_opt_names: HashSet<String> = matched_opts
            .iter()
            .map(|opt| opt.name().to_string())
            .collect();
        if !matched_opt_names.is_superset(&self.required_options) {
            let mut options = self
                .required_options
                .difference(&matched_opt_names)
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            options.sort();
            return Err(Error::MissingRequiredOptions {
                program: self.program.clone(),
                options,
            });
        }

        let exec = ValidExec {
            program: self.program.clone(),
            flags: matched_flags,
            opts: matched_opts,
            args: matched_args,
            system_path: self.system_path.clone(),
        };
        match &self.forbidden {
            Some(reason) => Ok(MatchedExec::Forbidden {
                cause: Forbidden::Exec { exec },
                reason: reason.clone(),
            }),
            None => Ok(MatchedExec::Match { exec }),
        }
    }

    pub fn verify_should_match_list(&self) -> Vec<PositiveExampleFailedCheck> {
        let mut violations = Vec::new();
        for good in &self.should_match {
            let exec_call = ExecCall {
                program: self.program.clone(),
                args: good.clone(),
            };
            match self.check(&exec_call) {
                Ok(_) => {}
                Err(error) => {
                    violations.push(PositiveExampleFailedCheck {
                        program: self.program.clone(),
                        args: good.clone(),
                        error,
                    });
                }
            }
        }
        violations
    }

    pub fn verify_should_not_match_list(&self) -> Vec<NegativeExamplePassedCheck> {
        let mut violations = Vec::new();
        for bad in &self.should_not_match {
            let exec_call = ExecCall {
                program: self.program.clone(),
                args: bad.clone(),
            };
            if self.check(&exec_call).is_ok() {
                violations.push(NegativeExamplePassedCheck {
                    program: self.program.clone(),
                    args: bad.clone(),
                });
            }
        }
        violations
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct PositiveExampleFailedCheck {
    pub program: String,
    pub args: Vec<String>,
    pub error: Error,
}

#[derive(Debug, Eq, PartialEq)]
pub struct NegativeExamplePassedCheck {
    pub program: String,
    pub args: Vec<String>,
}
