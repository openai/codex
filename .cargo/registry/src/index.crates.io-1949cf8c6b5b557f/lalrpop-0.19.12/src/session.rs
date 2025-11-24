//! Internal configuration and session-specific settings. This is similar
//! to `configuration::Configuration`, but it is not exported outside the
//! crate. Note that all fields are public and so forth for convenience.

use crate::log::{Level, Log};
use std::collections::BTreeSet;
use std::default::Default;
use std::path;
use crate::style::{self, Style};

// These two, ubiquitous types are defined here so that their fields can be private
// across crate, but visible within the crate:

#[derive(Copy, Clone)]
pub enum ColorConfig {
    /// Use ANSI colors.
    Yes,

    /// Do NOT use ANSI colors.
    No,

    /// Use them if we detect a TTY output (default).
    IfTty,
}

/// Various options to control debug output. Although this struct is
/// technically part of LALRPOP's exported interface, it is not
/// considered part of the semver guarantees as end-users are not
/// expected to use it.
#[derive(Clone)]
pub struct Session {
    pub log: Log,

    pub force_build: bool,

    pub in_dir: Option<path::PathBuf>,

    pub out_dir: Option<path::PathBuf>,

    /// Emit `rerun-if-changed` directives for Cargo
    pub emit_rerun_directives: bool,

    /// Emit comments in generated code explaining the states and so
    /// forth.
    pub emit_comments: bool,

    /// Emit whitespace in the generated code to improve readability.
    pub emit_whitespace: bool,

    /// Emit report file about generated code
    pub emit_report: bool,

    pub color_config: ColorConfig,

    /// Stop after you find `max_errors` errors. If this value is 0,
    /// report *all* errors. Note that we MAY always report more than
    /// this value if we so choose.
    pub max_errors: usize,

    // Styles to use when formatting error reports
    /// Applied to the heading in a message.
    pub heading: Style,

    /// Applied to symbols in an ambiguity report (where there is no cursor)
    pub ambig_symbols: Style,

    /// Applied to symbols before the cursor in a local ambiguity report
    pub observed_symbols: Style,

    /// Applied to symbols at the cursor in a local ambiguity report,
    /// if it is a non-terminal
    pub cursor_symbol: Style,

    /// Applied to symbols after the cursor in a local ambiguity report
    pub unobserved_symbols: Style,

    /// Applied to terminal symbols, in addition to the above styles
    pub terminal_symbol: Style,

    /// Applied to nonterminal symbols, in addition to the above styles
    pub nonterminal_symbol: Style,

    /// Style to use when printing "Hint:"
    pub hint_text: Style,

    /// Unit testing (lalrpop-test) configuration
    pub unit_test: bool,

    /// Features used for conditional compilation
    pub features: Option<BTreeSet<String>>,
}

impl Session {
    pub fn new() -> Session {
        Session {
            log: Log::new(Level::Informative),
            in_dir: None,
            out_dir: None,
            force_build: false,
            emit_rerun_directives: false,
            emit_comments: false,
            emit_whitespace: true,
            emit_report: false,
            color_config: ColorConfig::default(),
            max_errors: 1,
            heading: style::FG_WHITE.with(style::BOLD),
            ambig_symbols: style::FG_WHITE,
            observed_symbols: style::FG_BRIGHT_GREEN,
            cursor_symbol: style::FG_BRIGHT_WHITE,
            unobserved_symbols: style::FG_BRIGHT_RED,
            terminal_symbol: style::BOLD,
            nonterminal_symbol: style::DEFAULT,
            hint_text: style::FG_BRIGHT_MAGENTA.with(style::BOLD),
            unit_test: false,
            features: Default::default(),
        }
    }

    /// A session suitable for use in testing.
    #[cfg(test)]
    pub fn test() -> Session {
        Session {
            log: Log::new(Level::Debug),
            in_dir: None,
            out_dir: None,
            force_build: false,
            emit_rerun_directives: false,
            emit_comments: false,
            emit_whitespace: true,
            emit_report: false,
            color_config: ColorConfig::IfTty,
            max_errors: 1,
            heading: Style::new(),
            ambig_symbols: Style::new(),
            observed_symbols: Style::new(),
            cursor_symbol: Style::new(),
            unobserved_symbols: Style::new(),
            terminal_symbol: Style::new(),
            nonterminal_symbol: Style::new(),
            hint_text: Style::new(),
            unit_test: true,
            features: Default::default(),
        }
    }

    /// Indicates whether we should stop after `actual_errors` number
    /// of errors have been reported.
    pub fn stop_after(&self, actual_errors: usize) -> bool {
        self.max_errors != 0 && actual_errors >= self.max_errors
    }

    pub fn log<M>(&self, level: Level, message: M)
    where
        M: FnOnce() -> String,
    {
        self.log.log(level, message)
    }

    pub fn emit_rerun_directive(&self, path: &path::Path) {
        if self.emit_rerun_directives {
            if let Some(display) = path.to_str() {
                println!("cargo:rerun-if-changed={}", display)
            } else {
                println!("cargo:warning=LALRPOP is unable to inform Cargo that {} is a dependency because its filename cannot be represented in UTF-8. This is probably because it contains an unpaired surrogate character on Windows. As a result, your build script will not be rerun when it changes.", path.to_string_lossy());
            }
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Session::new()
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        ColorConfig::IfTty
    }
}
