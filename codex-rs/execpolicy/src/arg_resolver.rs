use serde::Serialize;

use crate::arg_matcher::ArgMatcher;
use crate::arg_matcher::ArgMatcherCardinality;
use crate::error::Error;
use crate::error::Result;
use crate::valid_exec::MatchedArg;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PositionalArg {
    pub index: usize,
    pub value: String,
}

pub fn resolve_observed_args_with_patterns(
    program: &str,
    args: Vec<PositionalArg>,
    arg_patterns: &Vec<ArgMatcher>,
) -> Result<Vec<MatchedArg>> {
    // Naive matching implementation. Among `arg_patterns`, there is allowed to
    // be at most one vararg pattern. Assuming `arg_patterns` is non-empty, we
    // end up with either:
    //
    // - all `arg_patterns` in `prefix_patterns`
    // - `arg_patterns` split across `prefix_patterns` (which could be empty),
    //   one `vararg_pattern`, and `suffix_patterns` (which could also empty).
    //
    // From there, we start by matching everything in `prefix_patterns`.
    // Then we calculate how many positional args should be matched by
    // `suffix_patterns` and use that to determine how many args are left to
    // be matched by `vararg_pattern` (which could be zero).
    //
    // After assocating positional args with `vararg_pattern`, we match the
    // `suffix_patterns` with the remaining args.
    let ParitionedArgs {
        num_prefix_args,
        num_suffix_args,
        prefix_patterns,
        suffix_patterns,
        vararg_pattern,
    } = partition_args(program, arg_patterns)?;

    let mut matched_args = Vec::<MatchedArg>::new();

    let prefix = get_range_checked(&args, 0..num_prefix_args)?;
    let mut prefix_arg_index = 0;
    for pattern in prefix_patterns {
        let n = pattern
            .cardinality()
            .is_exact()
            .ok_or(Error::InternalInvariantViolation {
                message: "expected exact cardinality".to_string(),
            })?;
        for positional_arg in &prefix[prefix_arg_index..prefix_arg_index + n] {
            let matched_arg = MatchedArg::new(
                positional_arg.index,
                pattern.arg_type(),
                &positional_arg.value.clone(),
            )?;
            matched_args.push(matched_arg);
        }
        prefix_arg_index += n;
    }

    if num_suffix_args > args.len() {
        return Err(Error::NotEnoughArgs {
            program: program.to_string(),
            args,
            arg_patterns: arg_patterns.clone(),
        });
    }

    let initial_suffix_args_index = args.len() - num_suffix_args;
    if prefix_arg_index > initial_suffix_args_index {
        return Err(Error::PrefixOverlapsSuffix {});
    }

    if let Some(pattern) = vararg_pattern {
        let vararg = get_range_checked(&args, prefix_arg_index..initial_suffix_args_index)?;
        match pattern.cardinality() {
            ArgMatcherCardinality::One => {
                return Err(Error::InternalInvariantViolation {
                    message: "vararg pattern should not have cardinality of one".to_string(),
                });
            }
            ArgMatcherCardinality::AtLeastOne => {
                if vararg.is_empty() {
                    return Err(Error::VarargMatcherDidNotMatchAnything {
                        program: program.to_string(),
                        matcher: pattern,
                    });
                } else {
                    for positional_arg in vararg {
                        let matched_arg = MatchedArg::new(
                            positional_arg.index,
                            pattern.arg_type(),
                            &positional_arg.value.clone(),
                        )?;
                        matched_args.push(matched_arg);
                    }
                }
            }
            ArgMatcherCardinality::ZeroOrMore => {
                for positional_arg in vararg {
                    let matched_arg = MatchedArg::new(
                        positional_arg.index,
                        pattern.arg_type(),
                        &positional_arg.value.clone(),
                    )?;
                    matched_args.push(matched_arg);
                }
            }
        }
    }

    let suffix = get_range_checked(&args, initial_suffix_args_index..args.len())?;
    let mut suffix_arg_index = 0;
    for pattern in suffix_patterns {
        let n = pattern
            .cardinality()
            .is_exact()
            .ok_or(Error::InternalInvariantViolation {
                message: "expected exact cardinality".to_string(),
            })?;
        for positional_arg in &suffix[suffix_arg_index..suffix_arg_index + n] {
            let matched_arg = MatchedArg::new(
                positional_arg.index,
                pattern.arg_type(),
                &positional_arg.value.clone(),
            )?;
            matched_args.push(matched_arg);
        }
        suffix_arg_index += n;
    }

    if matched_args.len() < args.len() {
        let extra_args = get_range_checked(&args, matched_args.len()..args.len())?;
        Err(Error::UnexpectedArguments {
            program: program.to_string(),
            args: extra_args.to_vec(),
        })
    } else {
        Ok(matched_args)
    }
}

#[derive(Default)]
struct ParitionedArgs {
    num_prefix_args: usize,
    num_suffix_args: usize,
    prefix_patterns: Vec<ArgMatcher>,
    suffix_patterns: Vec<ArgMatcher>,
    vararg_pattern: Option<ArgMatcher>,
}

fn partition_args(program: &str, arg_patterns: &Vec<ArgMatcher>) -> Result<ParitionedArgs> {
    let mut in_prefix = true;
    let mut partitioned_args = ParitionedArgs::default();

    for pattern in arg_patterns {
        match pattern.cardinality().is_exact() {
            Some(n) => {
                if in_prefix {
                    partitioned_args.prefix_patterns.push(pattern.clone());
                    partitioned_args.num_prefix_args += n;
                } else {
                    partitioned_args.suffix_patterns.push(pattern.clone());
                    partitioned_args.num_suffix_args += n;
                }
            }
            None => match partitioned_args.vararg_pattern {
                None => {
                    partitioned_args.vararg_pattern = Some(pattern.clone());
                    in_prefix = false;
                }
                Some(existing_pattern) => {
                    return Err(Error::MultipleVarargPatterns {
                        program: program.to_string(),
                        first: existing_pattern,
                        second: pattern.clone(),
                    });
                }
            },
        }
    }

    Ok(partitioned_args)
}

fn get_range_checked<T>(vec: &[T], range: std::ops::Range<usize>) -> Result<&[T]> {
    if range.start > range.end {
        Err(Error::RangeStartExceedsEnd {
            start: range.start,
            end: range.end,
        })
    } else if range.end > vec.len() {
        Err(Error::RangeEndOutOfBounds {
            end: range.end,
            len: vec.len(),
        })
    } else {
        Ok(&vec[range])
    }
}
