// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// ignore-lexer-test FIXME #15677

//! Simple getopt alternative.
//!
//! Construct instance of `Options` and configure it by using  `reqopt()`,
//! `optopt()` and other methods that add option configuration. Then call
//! `parse()` method and pass into it a vector of actual arguments (not
//! including `argv[0]`).
//!
//! You'll either get a failure code back, or a match. You'll have to verify
//! whether the amount of 'free' arguments in the match is what you expect. Use
//! `opt_*` accessors to get argument values out of the matches object.
//!
//! Single-character options are expected to appear on the command line with a
//! single preceding dash; multiple-character options are expected to be
//! proceeded by two dashes. Options that expect an argument accept their
//! argument following either a space or an equals sign. Single-character
//! options don't require the space. Everything after double-dash "--"  argument
//! is considered to be a 'free' argument, even if it starts with dash.
//!
//! # Usage
//!
//! This crate is [on crates.io](https://crates.io/crates/getopts) and can be
//! used by adding `getopts` to the dependencies in your project's `Cargo.toml`.
//!
//! ```toml
//! [dependencies]
//! getopts = "0.2"
//! ```
//!
//! and this to your crate root:
//!
//! ```rust
//! extern crate getopts;
//! ```
//!
//! # Example
//!
//! The following example shows simple command line parsing for an application
//! that requires an input file to be specified, accepts an optional output file
//! name following `-o`, and accepts both `-h` and `--help` as optional flags.
//!
//! ```{.rust}
//! extern crate getopts;
//! use getopts::Options;
//! use std::env;
//!
//! fn do_work(inp: &str, out: Option<String>) {
//!     println!("{}", inp);
//!     match out {
//!         Some(x) => println!("{}", x),
//!         None => println!("No Output"),
//!     }
//! }
//!
//! fn print_usage(program: &str, opts: Options) {
//!     let brief = format!("Usage: {} FILE [options]", program);
//!     print!("{}", opts.usage(&brief));
//! }
//!
//! fn main() {
//!     let args: Vec<String> = env::args().collect();
//!     let program = args[0].clone();
//!
//!     let mut opts = Options::new();
//!     opts.optopt("o", "", "set output file name", "NAME");
//!     opts.optflag("h", "help", "print this help menu");
//!     let matches = match opts.parse(&args[1..]) {
//!         Ok(m) => { m }
//!         Err(f) => { panic!("{}", f.to_string()) }
//!     };
//!     if matches.opt_present("h") {
//!         print_usage(&program, opts);
//!         return;
//!     }
//!     let output = matches.opt_str("o");
//!     let input = if !matches.free.is_empty() {
//!         matches.free[0].clone()
//!     } else {
//!         print_usage(&program, opts);
//!         return;
//!     };
//!     do_work(&input, output);
//! }
//! ```

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico"
)]
#![deny(missing_docs)]
#![cfg_attr(test, deny(warnings))]

#[cfg(test)]
#[macro_use]
extern crate log;
extern crate unicode_width;

use self::Fail::*;
use self::HasArg::*;
use self::Name::*;
use self::Occur::*;
use self::Optval::*;

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::iter::{repeat, IntoIterator};
use std::result;
use std::str::FromStr;

use unicode_width::UnicodeWidthStr;

#[cfg(test)]
mod tests;

/// A description of the options that a program can handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    grps: Vec<OptGroup>,
    parsing_style: ParsingStyle,
    long_only: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self::new()
    }
}

impl Options {
    /// Create a blank set of options.
    pub fn new() -> Options {
        Options {
            grps: Vec::new(),
            parsing_style: ParsingStyle::FloatingFrees,
            long_only: false,
        }
    }

    /// Set the parsing style.
    pub fn parsing_style(&mut self, style: ParsingStyle) -> &mut Options {
        self.parsing_style = style;
        self
    }

    /// Set or clear "long options only" mode.
    ///
    /// In "long options only" mode, short options cannot be clustered
    /// together, and long options can be given with either a single
    /// "-" or the customary "--".  This mode also changes the meaning
    /// of "-a=b"; in the ordinary mode this will parse a short option
    /// "-a" with argument "=b"; whereas in long-options-only mode the
    /// argument will be simply "b".
    pub fn long_only(&mut self, long_only: bool) -> &mut Options {
        self.long_only = long_only;
        self
    }

    /// Create a generic option group, stating all parameters explicitly.
    pub fn opt(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
        hint: &str,
        hasarg: HasArg,
        occur: Occur,
    ) -> &mut Options {
        validate_names(short_name, long_name);
        self.grps.push(OptGroup {
            short_name: short_name.to_string(),
            long_name: long_name.to_string(),
            hint: hint.to_string(),
            desc: desc.to_string(),
            hasarg,
            occur,
        });
        self
    }

    /// Create a long option that is optional and does not take an argument.
    ///
    /// * `short_name` - e.g. `"h"` for a `-h` option, or `""` for none
    /// * `long_name` - e.g. `"help"` for a `--help` option, or `""` for none
    /// * `desc` - Description for usage help
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    /// opts.optflag("h", "help", "help flag");
    ///
    /// let matches = opts.parse(&["-h"]).unwrap();
    /// assert!(matches.opt_present("h"));
    /// ```
    pub fn optflag(&mut self, short_name: &str, long_name: &str, desc: &str) -> &mut Options {
        validate_names(short_name, long_name);
        self.grps.push(OptGroup {
            short_name: short_name.to_string(),
            long_name: long_name.to_string(),
            hint: "".to_string(),
            desc: desc.to_string(),
            hasarg: No,
            occur: Optional,
        });
        self
    }

    /// Create a long option that can occur more than once and does not
    /// take an argument.
    ///
    /// * `short_name` - e.g. `"h"` for a `-h` option, or `""` for none
    /// * `long_name` - e.g. `"help"` for a `--help` option, or `""` for none
    /// * `desc` - Description for usage help
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    /// opts.optflagmulti("v", "verbose", "verbosity flag");
    ///
    /// let matches = opts.parse(&["-v", "--verbose"]).unwrap();
    /// assert_eq!(2, matches.opt_count("v"));
    /// ```
    pub fn optflagmulti(&mut self, short_name: &str, long_name: &str, desc: &str) -> &mut Options {
        validate_names(short_name, long_name);
        self.grps.push(OptGroup {
            short_name: short_name.to_string(),
            long_name: long_name.to_string(),
            hint: "".to_string(),
            desc: desc.to_string(),
            hasarg: No,
            occur: Multi,
        });
        self
    }

    /// Create a long option that is optional and takes an optional argument.
    ///
    /// * `short_name` - e.g. `"h"` for a `-h` option, or `""` for none
    /// * `long_name` - e.g. `"help"` for a `--help` option, or `""` for none
    /// * `desc` - Description for usage help
    /// * `hint` - Hint that is used in place of the argument in the usage help,
    ///   e.g. `"FILE"` for a `-o FILE` option
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    /// opts.optflagopt("t", "text", "flag with optional argument", "TEXT");
    ///
    /// let matches = opts.parse(&["--text"]).unwrap();
    /// assert_eq!(None, matches.opt_str("text"));
    ///
    /// let matches = opts.parse(&["--text=foo"]).unwrap();
    /// assert_eq!(Some("foo".to_owned()), matches.opt_str("text"));
    /// ```
    pub fn optflagopt(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
        hint: &str,
    ) -> &mut Options {
        validate_names(short_name, long_name);
        self.grps.push(OptGroup {
            short_name: short_name.to_string(),
            long_name: long_name.to_string(),
            hint: hint.to_string(),
            desc: desc.to_string(),
            hasarg: Maybe,
            occur: Optional,
        });
        self
    }

    /// Create a long option that is optional, takes an argument, and may occur
    /// multiple times.
    ///
    /// * `short_name` - e.g. `"h"` for a `-h` option, or `""` for none
    /// * `long_name` - e.g. `"help"` for a `--help` option, or `""` for none
    /// * `desc` - Description for usage help
    /// * `hint` - Hint that is used in place of the argument in the usage help,
    ///   e.g. `"FILE"` for a `-o FILE` option
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    /// opts.optmulti("t", "text", "text option", "TEXT");
    ///
    /// let matches = opts.parse(&["-t", "foo", "--text=bar"]).unwrap();
    ///
    /// let values = matches.opt_strs("t");
    /// assert_eq!(2, values.len());
    /// assert_eq!("foo", values[0]);
    /// assert_eq!("bar", values[1]);
    /// ```
    pub fn optmulti(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
        hint: &str,
    ) -> &mut Options {
        validate_names(short_name, long_name);
        self.grps.push(OptGroup {
            short_name: short_name.to_string(),
            long_name: long_name.to_string(),
            hint: hint.to_string(),
            desc: desc.to_string(),
            hasarg: Yes,
            occur: Multi,
        });
        self
    }

    /// Create a long option that is optional and takes an argument.
    ///
    /// * `short_name` - e.g. `"h"` for a `-h` option, or `""` for none
    /// * `long_name` - e.g. `"help"` for a `--help` option, or `""` for none
    /// * `desc` - Description for usage help
    /// * `hint` - Hint that is used in place of the argument in the usage help,
    ///   e.g. `"FILE"` for a `-o FILE` option
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// # use getopts::Fail;
    /// let mut opts = Options::new();
    /// opts.optopt("o", "optional", "optional text option", "TEXT");
    ///
    /// let matches = opts.parse(&["arg1"]).unwrap();
    /// assert_eq!(None, matches.opt_str("optional"));
    ///
    /// let matches = opts.parse(&["--optional", "foo", "arg1"]).unwrap();
    /// assert_eq!(Some("foo".to_owned()), matches.opt_str("optional"));
    /// ```
    pub fn optopt(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
        hint: &str,
    ) -> &mut Options {
        validate_names(short_name, long_name);
        self.grps.push(OptGroup {
            short_name: short_name.to_string(),
            long_name: long_name.to_string(),
            hint: hint.to_string(),
            desc: desc.to_string(),
            hasarg: Yes,
            occur: Optional,
        });
        self
    }

    /// Create a long option that is required and takes an argument.
    ///
    /// * `short_name` - e.g. `"h"` for a `-h` option, or `""` for none
    /// * `long_name` - e.g. `"help"` for a `--help` option, or `""` for none
    /// * `desc` - Description for usage help
    /// * `hint` - Hint that is used in place of the argument in the usage help,
    ///   e.g. `"FILE"` for a `-o FILE` option
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// # use getopts::Fail;
    /// let mut opts = Options::new();
    /// opts.optopt("o", "optional", "optional text option", "TEXT");
    /// opts.reqopt("m", "mandatory", "madatory text option", "TEXT");
    ///
    /// let result = opts.parse(&["--mandatory", "foo"]);
    /// assert!(result.is_ok());
    ///
    /// let result = opts.parse(&["--optional", "foo"]);
    /// assert!(result.is_err());
    /// assert_eq!(Fail::OptionMissing("mandatory".to_owned()), result.unwrap_err());
    /// ```
    pub fn reqopt(
        &mut self,
        short_name: &str,
        long_name: &str,
        desc: &str,
        hint: &str,
    ) -> &mut Options {
        validate_names(short_name, long_name);
        self.grps.push(OptGroup {
            short_name: short_name.to_string(),
            long_name: long_name.to_string(),
            hint: hint.to_string(),
            desc: desc.to_string(),
            hasarg: Yes,
            occur: Req,
        });
        self
    }

    /// Parse command line arguments according to the provided options.
    ///
    /// On success returns `Ok(Matches)`. Use methods such as `opt_present`
    /// `opt_str`, etc. to interrogate results.
    /// # Panics
    ///
    /// Returns `Err(Fail)` on failure: use the `Debug` implementation of `Fail`
    /// to display information about it.
    pub fn parse<C: IntoIterator>(&self, args: C) -> Result
    where
        C::Item: AsRef<OsStr>,
    {
        let opts: Vec<Opt> = self.grps.iter().map(|x| x.long_to_short()).collect();

        let mut vals = (0..opts.len())
            .map(|_| Vec::new())
            .collect::<Vec<Vec<(usize, Optval)>>>();
        let mut free: Vec<String> = Vec::new();
        let mut args_end = None;

        let args = args
            .into_iter()
            .map(|i| {
                i.as_ref()
                    .to_str()
                    .ok_or_else(|| Fail::UnrecognizedOption(format!("{:?}", i.as_ref())))
                    .map(|s| s.to_owned())
            })
            .collect::<::std::result::Result<Vec<_>, _>>()?;
        let mut args = args.into_iter().peekable();
        let mut arg_pos = 0;
        while let Some(cur) = args.next() {
            if !is_arg(&cur) {
                free.push(cur);
                match self.parsing_style {
                    ParsingStyle::FloatingFrees => {}
                    ParsingStyle::StopAtFirstFree => {
                        free.extend(args);
                        break;
                    }
                }
            } else if cur == "--" {
                args_end = Some(free.len());
                free.extend(args);
                break;
            } else {
                let mut name = None;
                let mut i_arg = None;
                let mut was_long = true;
                if cur.as_bytes()[1] == b'-' || self.long_only {
                    let tail = if cur.as_bytes()[1] == b'-' {
                        &cur[2..]
                    } else {
                        assert!(self.long_only);
                        &cur[1..]
                    };
                    let mut parts = tail.splitn(2, '=');
                    name = Some(Name::from_str(parts.next().unwrap()));
                    if let Some(rest) = parts.next() {
                        i_arg = Some(rest.to_string());
                    }
                } else {
                    was_long = false;
                    for (j, ch) in cur.char_indices().skip(1) {
                        let opt = Short(ch);

                        let opt_id = match find_opt(&opts, &opt) {
                            Some(id) => id,
                            None => return Err(UnrecognizedOption(opt.to_string())),
                        };

                        // In a series of potential options (eg. -aheJ), if we
                        // see one which takes an argument, we assume all
                        // subsequent characters make up the argument. This
                        // allows options such as -L/usr/local/lib/foo to be
                        // interpreted correctly
                        let arg_follows = match opts[opt_id].hasarg {
                            Yes | Maybe => true,
                            No => false,
                        };

                        if arg_follows {
                            name = Some(opt);
                            let next = j + ch.len_utf8();
                            if next < cur.len() {
                                i_arg = Some(cur[next..].to_string());
                                break;
                            }
                        } else {
                            vals[opt_id].push((arg_pos, Given));
                        }
                    }
                }
                if let Some(nm) = name {
                    let opt_id = match find_opt(&opts, &nm) {
                        Some(id) => id,
                        None => return Err(UnrecognizedOption(nm.to_string())),
                    };
                    match opts[opt_id].hasarg {
                        No => {
                            if i_arg.is_some() {
                                return Err(UnexpectedArgument(nm.to_string()));
                            }
                            vals[opt_id].push((arg_pos, Given));
                        }
                        Maybe => {
                            // Note that here we do not handle `--arg value`.
                            // This matches GNU getopt behavior; but also
                            // makes sense, because if this were accepted,
                            // then users could only write a "Maybe" long
                            // option at the end of the arguments when
                            // FloatingFrees is in use.
                            if let Some(i_arg) = i_arg.take() {
                                vals[opt_id].push((arg_pos, Val(i_arg)));
                            } else if was_long || args.peek().map_or(true, |n| is_arg(&n)) {
                                vals[opt_id].push((arg_pos, Given));
                            } else {
                                vals[opt_id].push((arg_pos, Val(args.next().unwrap())));
                            }
                        }
                        Yes => {
                            if let Some(i_arg) = i_arg.take() {
                                vals[opt_id].push((arg_pos, Val(i_arg)));
                            } else if let Some(n) = args.next() {
                                vals[opt_id].push((arg_pos, Val(n)));
                            } else {
                                return Err(ArgumentMissing(nm.to_string()));
                            }
                        }
                    }
                }
            }
            arg_pos += 1;
        }
        debug_assert_eq!(vals.len(), opts.len());
        for (vals, opt) in vals.iter().zip(opts.iter()) {
            if opt.occur == Req && vals.is_empty() {
                return Err(OptionMissing(opt.name.to_string()));
            }
            if opt.occur != Multi && vals.len() > 1 {
                return Err(OptionDuplicated(opt.name.to_string()));
            }
        }

        // Note that if "--" is last argument on command line, then index stored
        // in option does not exist in `free` and must be replaced with `None`
        args_end = args_end.filter(|pos| pos != &free.len());

        Ok(Matches {
            opts,
            vals,
            free,
            args_end,
        })
    }

    /// Derive a short one-line usage summary from a set of long options.
    pub fn short_usage(&self, program_name: &str) -> String {
        let mut line = format!("Usage: {} ", program_name);
        line.push_str(
            &self
                .grps
                .iter()
                .map(format_option)
                .collect::<Vec<String>>()
                .join(" "),
        );
        line
    }

    /// Derive a formatted message from a set of options.
    pub fn usage(&self, brief: &str) -> String {
        self.usage_with_format(|opts| {
            format!(
                "{}\n\nOptions:\n{}\n",
                brief,
                opts.collect::<Vec<String>>().join("\n")
            )
        })
    }

    /// Derive a custom formatted message from a set of options. The formatted options provided to
    /// a closure as an iterator.
    pub fn usage_with_format<F: FnMut(&mut dyn Iterator<Item = String>) -> String>(
        &self,
        mut formatter: F,
    ) -> String {
        formatter(&mut self.usage_items())
    }

    /// Derive usage items from a set of options.
    fn usage_items<'a>(&'a self) -> Box<dyn Iterator<Item = String> + 'a> {
        let desc_sep = format!("\n{}", repeat(" ").take(24).collect::<String>());

        let any_short = self.grps.iter().any(|optref| !optref.short_name.is_empty());

        let rows = self.grps.iter().map(move |optref| {
            let OptGroup {
                short_name,
                long_name,
                hint,
                desc,
                hasarg,
                ..
            } = (*optref).clone();

            let mut row = "    ".to_string();

            // short option
            match short_name.width() {
                0 => {
                    if any_short {
                        row.push_str("    ");
                    }
                }
                1 => {
                    row.push('-');
                    row.push_str(&short_name);
                    if long_name.width() > 0 {
                        row.push_str(", ");
                    } else {
                        // Only a single space here, so that any
                        // argument is printed in the correct spot.
                        row.push(' ');
                    }
                }
                // FIXME: refer issue #7.
                _ => panic!("the short name should only be 1 ascii char long"),
            }

            // long option
            match long_name.width() {
                0 => {}
                _ => {
                    row.push_str(if self.long_only { "-" } else { "--" });
                    row.push_str(&long_name);
                    row.push(' ');
                }
            }

            // arg
            match hasarg {
                No => {}
                Yes => row.push_str(&hint),
                Maybe => {
                    row.push('[');
                    row.push_str(&hint);
                    row.push(']');
                }
            }

            let rowlen = row.width();
            if rowlen < 24 {
                for _ in 0..24 - rowlen {
                    row.push(' ');
                }
            } else {
                row.push_str(&desc_sep)
            }

            let desc_rows = each_split_within(&desc, 54);
            row.push_str(&desc_rows.join(&desc_sep));

            row
        });

        Box::new(rows)
    }
}

fn validate_names(short_name: &str, long_name: &str) {
    let len = short_name.len();
    assert!(
        len == 1 || len == 0,
        "the short_name (first argument) should be a single character, \
         or an empty string for none"
    );
    let len = long_name.len();
    assert!(
        len == 0 || len > 1,
        "the long_name (second argument) should be longer than a single \
         character, or an empty string for none"
    );
}

/// What parsing style to use when parsing arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsingStyle {
    /// Flags and "free" arguments can be freely inter-mixed.
    FloatingFrees,
    /// As soon as a "free" argument (i.e. non-flag) is encountered, stop
    /// considering any remaining arguments as flags.
    StopAtFirstFree,
}

/// Name of an option. Either a string or a single char.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Name {
    /// A string representing the long name of an option.
    /// For example: "help"
    Long(String),
    /// A char representing the short name of an option.
    /// For example: 'h'
    Short(char),
}

/// Describes whether an option has an argument.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum HasArg {
    /// The option requires an argument.
    Yes,
    /// The option takes no argument.
    No,
    /// The option argument is optional.
    Maybe,
}

/// Describes how often an option may occur.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum Occur {
    /// The option occurs once.
    Req,
    /// The option occurs at most once.
    Optional,
    /// The option occurs zero or more times.
    Multi,
}

/// A description of a possible option.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Opt {
    /// Name of the option
    name: Name,
    /// Whether it has an argument
    hasarg: HasArg,
    /// How often it can occur
    occur: Occur,
    /// Which options it aliases
    aliases: Vec<Opt>,
}

/// One group of options, e.g., both `-h` and `--help`, along with
/// their shared description and properties.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OptGroup {
    /// Short name of the option, e.g. `h` for a `-h` option
    short_name: String,
    /// Long name of the option, e.g. `help` for a `--help` option
    long_name: String,
    /// Hint for argument, e.g. `FILE` for a `-o FILE` option
    hint: String,
    /// Description for usage help text
    desc: String,
    /// Whether option has an argument
    hasarg: HasArg,
    /// How often it can occur
    occur: Occur,
}

/// Describes whether an option is given at all or has a value.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Optval {
    Val(String),
    Given,
}

/// The result of checking command line arguments. Contains a vector
/// of matches and a vector of free strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Matches {
    /// Options that matched
    opts: Vec<Opt>,
    /// Values of the Options that matched and their positions
    vals: Vec<Vec<(usize, Optval)>>,

    /// Free string fragments
    pub free: Vec<String>,

    /// Index of first free fragment after "--" separator
    args_end: Option<usize>,
}

/// The type returned when the command line does not conform to the
/// expected format. Use the `Debug` implementation to output detailed
/// information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fail {
    /// The option requires an argument but none was passed.
    ArgumentMissing(String),
    /// The passed option is not declared among the possible options.
    UnrecognizedOption(String),
    /// A required option is not present.
    OptionMissing(String),
    /// A single occurrence option is being used multiple times.
    OptionDuplicated(String),
    /// There's an argument being passed to a non-argument option.
    UnexpectedArgument(String),
}

impl Error for Fail {}

/// The result of parsing a command line with a set of options.
pub type Result = result::Result<Matches, Fail>;

impl Name {
    fn from_str(nm: &str) -> Name {
        if nm.len() == 1 {
            Short(nm.as_bytes()[0] as char)
        } else {
            Long(nm.to_string())
        }
    }

    fn to_string(&self) -> String {
        match *self {
            Short(ch) => ch.to_string(),
            Long(ref s) => s.to_string(),
        }
    }
}

impl OptGroup {
    /// Translate OptGroup into Opt.
    /// (Both short and long names correspond to different Opts).
    fn long_to_short(&self) -> Opt {
        let OptGroup {
            short_name,
            long_name,
            hasarg,
            occur,
            ..
        } = (*self).clone();

        match (short_name.len(), long_name.len()) {
            (0, 0) => panic!("this long-format option was given no name"),
            (0, _) => Opt {
                name: Long(long_name),
                hasarg,
                occur,
                aliases: Vec::new(),
            },
            (1, 0) => Opt {
                name: Short(short_name.as_bytes()[0] as char),
                hasarg,
                occur,
                aliases: Vec::new(),
            },
            (1, _) => Opt {
                name: Long(long_name),
                hasarg,
                occur,
                aliases: vec![Opt {
                    name: Short(short_name.as_bytes()[0] as char),
                    hasarg: hasarg,
                    occur: occur,
                    aliases: Vec::new(),
                }],
            },
            (_, _) => panic!("something is wrong with the long-form opt"),
        }
    }
}

impl Matches {
    fn opt_vals(&self, nm: &str) -> Vec<(usize, Optval)> {
        match find_opt(&self.opts, &Name::from_str(nm)) {
            Some(id) => self.vals[id].clone(),
            None => panic!("No option '{}' defined", nm),
        }
    }

    fn opt_val(&self, nm: &str) -> Option<Optval> {
        self.opt_vals(nm).into_iter().map(|(_, o)| o).next()
    }
    /// Returns true if an option was defined
    pub fn opt_defined(&self, name: &str) -> bool {
        find_opt(&self.opts, &Name::from_str(name)).is_some()
    }

    /// Returns true if an option was matched.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_present(&self, name: &str) -> bool {
        !self.opt_vals(name).is_empty()
    }

    /// Returns the number of times an option was matched.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_count(&self, name: &str) -> usize {
        self.opt_vals(name).len()
    }

    /// Returns a vector of all the positions in which an option was matched.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_positions(&self, name: &str) -> Vec<usize> {
        self.opt_vals(name)
            .into_iter()
            .map(|(pos, _)| pos)
            .collect()
    }

    /// Returns true if any of several options were matched.
    pub fn opts_present(&self, names: &[String]) -> bool {
        names
            .iter()
            .any(|nm| match find_opt(&self.opts, &Name::from_str(&nm)) {
                Some(id) if !self.vals[id].is_empty() => true,
                _ => false,
            })
    }

    /// Returns true if any of several options were matched.
    ///
    /// Similar to `opts_present` but accepts any argument that can be converted
    /// into an iterator over string references.
    ///
    /// # Panics
    ///
    /// This function might panic if some option name is not defined.
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    /// opts.optopt("a", "alpha", "first option", "STR");
    /// opts.optopt("b", "beta", "second option", "STR");
    ///
    /// let args = vec!["-a", "foo"];
    /// let matches = &match opts.parse(&args) {
    ///     Ok(m) => m,
    ///     _ => panic!(),
    /// };
    ///
    /// assert!(matches.opts_present_any(&["alpha"]));
    /// assert!(!matches.opts_present_any(&["beta"]));
    /// ```
    pub fn opts_present_any<C: IntoIterator>(&self, names: C) -> bool
    where
        C::Item: AsRef<str>,
    {
        names
            .into_iter()
            .any(|nm| !self.opt_vals(nm.as_ref()).is_empty())
    }

    /// Returns the string argument supplied to one of several matching options or `None`.
    pub fn opts_str(&self, names: &[String]) -> Option<String> {
        names
            .iter()
            .filter_map(|nm| match self.opt_val(&nm) {
                Some(Val(s)) => Some(s),
                _ => None,
            })
            .next()
    }

    /// Returns the string argument supplied to the first matching option of
    /// several options or `None`.
    ///
    /// Similar to `opts_str` but accepts any argument that can be converted
    /// into an iterator over string references.
    ///
    /// # Panics
    ///
    /// This function might panic if some option name is not defined.
    ///
    /// # Example
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    /// opts.optopt("a", "alpha", "first option", "STR");
    /// opts.optopt("b", "beta", "second option", "STR");
    ///
    /// let args = vec!["-a", "foo", "--beta", "bar"];
    /// let matches = &match opts.parse(&args) {
    ///     Ok(m) => m,
    ///     _ => panic!(),
    /// };
    ///
    /// assert_eq!(Some("foo".to_string()), matches.opts_str_first(&["alpha", "beta"]));
    /// assert_eq!(Some("bar".to_string()), matches.opts_str_first(&["beta", "alpha"]));
    /// ```
    pub fn opts_str_first<C: IntoIterator>(&self, names: C) -> Option<String>
    where
        C::Item: AsRef<str>,
    {
        names
            .into_iter()
            .filter_map(|nm| match self.opt_val(nm.as_ref()) {
                Some(Val(s)) => Some(s),
                _ => None,
            })
            .next()
    }

    /// Returns a vector of the arguments provided to all matches of the given
    /// option.
    ///
    /// Used when an option accepts multiple values.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_strs(&self, name: &str) -> Vec<String> {
        self.opt_vals(name)
            .into_iter()
            .filter_map(|(_, v)| match v {
                Val(s) => Some(s),
                _ => None,
            })
            .collect()
    }

    /// Returns a vector of the arguments provided to all matches of the given
    /// option, together with their positions.
    ///
    /// Used when an option accepts multiple values.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_strs_pos(&self, name: &str) -> Vec<(usize, String)> {
        self.opt_vals(name)
            .into_iter()
            .filter_map(|(p, v)| match v {
                Val(s) => Some((p, s)),
                _ => None,
            })
            .collect()
    }

    /// Returns the string argument supplied to a matching option or `None`.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_str(&self, name: &str) -> Option<String> {
        match self.opt_val(name) {
            Some(Val(s)) => Some(s),
            _ => None,
        }
    }

    /// Returns the matching string, a default, or `None`.
    ///
    /// Returns `None` if the option was not present, `def` if the option was
    /// present but no argument was provided, and the argument if the option was
    /// present and an argument was provided.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_default(&self, name: &str, def: &str) -> Option<String> {
        match self.opt_val(name) {
            Some(Val(s)) => Some(s),
            Some(_) => Some(def.to_string()),
            None => None,
        }
    }

    /// Returns some matching value or `None`.
    ///
    /// Similar to opt_str, also converts matching argument using FromStr.
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_get<T>(&self, name: &str) -> result::Result<Option<T>, T::Err>
    where
        T: FromStr,
    {
        match self.opt_val(name) {
            Some(Val(s)) => Ok(Some(s.parse()?)),
            Some(Given) => Ok(None),
            None => Ok(None),
        }
    }

    /// Returns a matching value or default.
    ///
    /// Similar to opt_default, except the two differences.
    /// Instead of returning None when argument was not present, return `def`.
    /// Instead of returning &str return type T, parsed using str::parse().
    ///
    /// # Panics
    ///
    /// This function will panic if the option name is not defined.
    pub fn opt_get_default<T>(&self, name: &str, def: T) -> result::Result<T, T::Err>
    where
        T: FromStr,
    {
        match self.opt_val(name) {
            Some(Val(s)) => s.parse(),
            Some(Given) => Ok(def),
            None => Ok(def),
        }
    }

    /// Returns index of first free argument after "--".
    ///
    /// If double-dash separator is present and there are some args after it in
    /// the argument list then the method returns index into `free` vector
    /// indicating first argument after it.
    /// behind it.
    ///
    /// # Examples
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    ///
    /// let matches = opts.parse(&vec!["arg1", "--", "arg2"]).unwrap();
    /// let end_pos = matches.free_trailing_start().unwrap();
    /// assert_eq!(end_pos, 1);
    /// assert_eq!(matches.free[end_pos], "arg2".to_owned());
    /// ```
    ///
    /// If the double-dash is missing from argument list or if there are no
    /// arguments after it:
    ///
    /// ```
    /// # use getopts::Options;
    /// let mut opts = Options::new();
    ///
    /// let matches = opts.parse(&vec!["arg1", "--"]).unwrap();
    /// assert_eq!(matches.free_trailing_start(), None);
    ///
    /// let matches = opts.parse(&vec!["arg1", "arg2"]).unwrap();
    /// assert_eq!(matches.free_trailing_start(), None);
    /// ```
    ///
    pub fn free_trailing_start(&self) -> Option<usize> {
        self.args_end
    }
}

fn is_arg(arg: &str) -> bool {
    arg.as_bytes().get(0) == Some(&b'-') && arg.len() > 1
}

fn find_opt(opts: &[Opt], nm: &Name) -> Option<usize> {
    // Search main options.
    let pos = opts.iter().position(|opt| &opt.name == nm);
    if pos.is_some() {
        return pos;
    }

    // Search in aliases.
    for candidate in opts.iter() {
        if candidate.aliases.iter().any(|opt| &opt.name == nm) {
            return opts.iter().position(|opt| opt.name == candidate.name);
        }
    }

    None
}

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ArgumentMissing(ref nm) => write!(f, "Argument to option '{}' missing", *nm),
            UnrecognizedOption(ref nm) => write!(f, "Unrecognized option: '{}'", *nm),
            OptionMissing(ref nm) => write!(f, "Required option '{}' missing", *nm),
            OptionDuplicated(ref nm) => write!(f, "Option '{}' given more than once", *nm),
            UnexpectedArgument(ref nm) => write!(f, "Option '{}' does not take an argument", *nm),
        }
    }
}

fn format_option(opt: &OptGroup) -> String {
    let mut line = String::new();

    if opt.occur != Req {
        line.push('[');
    }

    // Use short_name if possible, but fall back to long_name.
    if !opt.short_name.is_empty() {
        line.push('-');
        line.push_str(&opt.short_name);
    } else {
        line.push_str("--");
        line.push_str(&opt.long_name);
    }

    if opt.hasarg != No {
        line.push(' ');
        if opt.hasarg == Maybe {
            line.push('[');
        }
        line.push_str(&opt.hint);
        if opt.hasarg == Maybe {
            line.push(']');
        }
    }

    if opt.occur != Req {
        line.push(']');
    }
    if opt.occur == Multi {
        line.push_str("..");
    }

    line
}

/// Splits a string into substrings with possibly internal whitespace,
/// each of them at most `lim` bytes long, if possible. The substrings
/// have leading and trailing whitespace removed, and are only cut at
/// whitespace boundaries.
fn each_split_within(desc: &str, lim: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for line in desc.trim().lines() {
        let line_chars = line.chars().chain(Some(' '));
        let words = line_chars
            .fold((Vec::new(), 0, 0), |(mut words, a, z), c| {
                let idx = z + c.len_utf8(); // Get the current byte offset

                // If the char is whitespace, advance the word start and maybe push a word
                if c.is_whitespace() {
                    if a != z {
                        words.push(&line[a..z]);
                    }
                    (words, idx, idx)
                }
                // If the char is not whitespace, continue, retaining the current
                else {
                    (words, a, idx)
                }
            })
            .0;

        let mut row = String::new();
        for word in words.iter() {
            let sep = if !row.is_empty() { Some(" ") } else { None };
            let width = row.width() + word.width() + sep.map(UnicodeWidthStr::width).unwrap_or(0);

            if width <= lim {
                if let Some(sep) = sep {
                    row.push_str(sep)
                }
                row.push_str(word);
                continue;
            }
            if !row.is_empty() {
                rows.push(row.clone());
                row.clear();
            }
            row.push_str(word);
        }
        if !row.is_empty() {
            rows.push(row);
        }
    }
    rows
}
