use multimap::MultiMap;
use regex_lite::Error as RegexError;
use regex_lite::Regex;

use crate::ExecCall;
use crate::Forbidden;
use crate::MatchedExec;
use crate::NegativeExamplePassedCheck;
use crate::ProgramSpec;
use crate::error::Error;
use crate::error::Result;
use crate::policy_parser::ForbiddenProgramRegex;
use crate::program::PositiveExampleFailedCheck;

pub struct Policy {
    programs: MultiMap<String, ProgramSpec>,
    forbidden_program_regexes: Vec<ForbiddenProgramRegex>,
    forbidden_substrings_pattern: Option<Regex>,
}

impl Policy {
    pub fn new(
        programs: MultiMap<String, ProgramSpec>,
        forbidden_program_regexes: Vec<ForbiddenProgramRegex>,
        forbidden_substrings: Vec<String>,
    ) -> std::result::Result<Self, RegexError> {
        let forbidden_substrings_pattern = if forbidden_substrings.is_empty() {
            None
        } else {
            let escaped_substrings = forbidden_substrings
                .iter()
                .map(|s| regex_lite::escape(s))
                .collect::<Vec<_>>()
                .join("|");
            Some(Regex::new(&format!("({escaped_substrings})"))?)
        };
        Ok(Self {
            programs,
            forbidden_program_regexes,
            forbidden_substrings_pattern,
        })
    }

    pub fn check(&self, exec_call: &ExecCall) -> Result<MatchedExec> {
        let ExecCall { program, args } = &exec_call;
        for ForbiddenProgramRegex { regex, reason } in &self.forbidden_program_regexes {
            if regex.is_match(program) {
                return Ok(MatchedExec::Forbidden {
                    cause: Forbidden::Program {
                        program: program.clone(),
                        exec_call: exec_call.clone(),
                    },
                    reason: reason.clone(),
                });
            }
        }

        for arg in args {
            if let Some(regex) = &self.forbidden_substrings_pattern
                && regex.is_match(arg)
            {
                return Ok(MatchedExec::Forbidden {
                    cause: Forbidden::Arg {
                        arg: arg.clone(),
                        exec_call: exec_call.clone(),
                    },
                    reason: format!("arg `{arg}` contains forbidden substring"),
                });
            }
        }

        let mut last_err = Err(Error::NoSpecForProgram {
            program: program.clone(),
        });
        if let Some(spec_list) = self.programs.get_vec(program) {
            for spec in spec_list {
                match spec.check(exec_call) {
                    Ok(matched_exec) => return Ok(matched_exec),
                    Err(err) => {
                        last_err = Err(err);
                    }
                }
            }
        }
        last_err
    }

    pub fn check_each_good_list_individually(&self) -> Vec<PositiveExampleFailedCheck> {
        let mut violations = Vec::new();
        for (_program, spec) in self.programs.flat_iter() {
            violations.extend(spec.verify_should_match_list());
        }
        violations
    }

    pub fn check_each_bad_list_individually(&self) -> Vec<NegativeExamplePassedCheck> {
        let mut violations = Vec::new();
        for (_program, spec) in self.programs.flat_iter() {
            violations.extend(spec.verify_should_not_match_list());
        }
        violations
    }
}
