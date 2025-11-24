// HACK: for rust 1.64 (1.68 doesn't need this since this is in lib.rs)
//
// Wanting consistency in our calls
#![allow(clippy::write_with_newline)]

// Std
use std::borrow::Cow;
use std::cmp;
use std::collections::BTreeMap;

// Internal
use crate::builder::PossibleValue;
use crate::builder::Str;
use crate::builder::StyledStr;
use crate::builder::Styles;
use crate::builder::{Arg, Command};
use crate::output::display_width;
use crate::output::wrap;
use crate::output::Usage;
use crate::output::TAB;
use crate::output::TAB_WIDTH;
use crate::util::FlatSet;

/// `clap` auto-generated help writer
pub(crate) struct AutoHelp<'cmd, 'writer> {
    template: HelpTemplate<'cmd, 'writer>,
}

// Public Functions
impl<'cmd, 'writer> AutoHelp<'cmd, 'writer> {
    /// Create a new `HelpTemplate` instance.
    pub(crate) fn new(
        writer: &'writer mut StyledStr,
        cmd: &'cmd Command,
        usage: &'cmd Usage<'cmd>,
        use_long: bool,
    ) -> Self {
        Self {
            template: HelpTemplate::new(writer, cmd, usage, use_long),
        }
    }

    pub(crate) fn write_help(&mut self) {
        let pos = self
            .template
            .cmd
            .get_positionals()
            .any(|arg| should_show_arg(self.template.use_long, arg));
        let non_pos = self
            .template
            .cmd
            .get_non_positionals()
            .any(|arg| should_show_arg(self.template.use_long, arg));
        let subcmds = self.template.cmd.has_visible_subcommands();

        let template = if non_pos || pos || subcmds {
            DEFAULT_TEMPLATE
        } else {
            DEFAULT_NO_ARGS_TEMPLATE
        };
        self.template.write_templated_help(template);
    }
}

const DEFAULT_TEMPLATE: &str = "\
{before-help}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}\
    ";

const DEFAULT_NO_ARGS_TEMPLATE: &str = "\
{before-help}{about-with-newline}
{usage-heading} {usage}{after-help}\
    ";

const SHORT_SIZE: usize = 4; // See `fn short` for the 4

/// Help template writer
///
/// Wraps a writer stream providing different methods to generate help for `clap` objects.
pub(crate) struct HelpTemplate<'cmd, 'writer> {
    writer: &'writer mut StyledStr,
    cmd: &'cmd Command,
    styles: &'cmd Styles,
    usage: &'cmd Usage<'cmd>,
    next_line_help: bool,
    term_w: usize,
    use_long: bool,
}

// Public Functions
impl<'cmd, 'writer> HelpTemplate<'cmd, 'writer> {
    /// Create a new `HelpTemplate` instance.
    pub(crate) fn new(
        writer: &'writer mut StyledStr,
        cmd: &'cmd Command,
        usage: &'cmd Usage<'cmd>,
        use_long: bool,
    ) -> Self {
        debug!(
            "HelpTemplate::new cmd={}, use_long={}",
            cmd.get_name(),
            use_long
        );
        let term_w = Self::term_w(cmd);
        let next_line_help = cmd.is_next_line_help_set();

        HelpTemplate {
            writer,
            cmd,
            styles: cmd.get_styles(),
            usage,
            next_line_help,
            term_w,
            use_long,
        }
    }

    #[cfg(not(feature = "unstable-v5"))]
    fn term_w(cmd: &'cmd Command) -> usize {
        match cmd.get_term_width() {
            Some(0) => usize::MAX,
            Some(w) => w,
            None => {
                let (current_width, _h) = dimensions();
                let current_width = current_width.unwrap_or(100);
                let max_width = match cmd.get_max_term_width() {
                    None | Some(0) => usize::MAX,
                    Some(mw) => mw,
                };
                cmp::min(current_width, max_width)
            }
        }
    }

    #[cfg(feature = "unstable-v5")]
    fn term_w(cmd: &'cmd Command) -> usize {
        let term_w = match cmd.get_term_width() {
            Some(0) => usize::MAX,
            Some(w) => w,
            None => {
                let (current_width, _h) = dimensions();
                current_width.unwrap_or(usize::MAX)
            }
        };

        let max_term_w = match cmd.get_max_term_width() {
            Some(0) => usize::MAX,
            Some(mw) => mw,
            None => 100,
        };

        cmp::min(term_w, max_term_w)
    }

    /// Write help to stream for the parser in the format defined by the template.
    ///
    /// For details about the template language see [`Command::help_template`].
    ///
    /// [`Command::help_template`]: Command::help_template()
    pub(crate) fn write_templated_help(&mut self, template: &str) {
        debug!("HelpTemplate::write_templated_help");
        use std::fmt::Write as _;

        let mut parts = template.split('{');
        if let Some(first) = parts.next() {
            self.writer.push_str(first);
        }
        for part in parts {
            if let Some((tag, rest)) = part.split_once('}') {
                match tag {
                    "name" => {
                        self.write_display_name();
                    }
                    #[cfg(not(feature = "unstable-v5"))]
                    "bin" => {
                        self.write_bin_name();
                    }
                    "version" => {
                        self.write_version();
                    }
                    "author" => {
                        self.write_author(false, false);
                    }
                    "author-with-newline" => {
                        self.write_author(false, true);
                    }
                    "author-section" => {
                        self.write_author(true, true);
                    }
                    "about" => {
                        self.write_about(false, false);
                    }
                    "about-with-newline" => {
                        self.write_about(false, true);
                    }
                    "about-section" => {
                        self.write_about(true, true);
                    }
                    "usage-heading" => {
                        let _ = write!(
                            self.writer,
                            "{}Usage:{}",
                            self.styles.get_usage().render(),
                            self.styles.get_usage().render_reset()
                        );
                    }
                    "usage" => {
                        self.writer.push_styled(
                            &self.usage.create_usage_no_title(&[]).unwrap_or_default(),
                        );
                    }
                    "all-args" => {
                        self.write_all_args();
                    }
                    "options" => {
                        // Include even those with a heading as we don't have a good way of
                        // handling help_heading in the template.
                        self.write_args(
                            &self.cmd.get_non_positionals().collect::<Vec<_>>(),
                            "options",
                            option_sort_key,
                        );
                    }
                    "positionals" => {
                        self.write_args(
                            &self.cmd.get_positionals().collect::<Vec<_>>(),
                            "positionals",
                            positional_sort_key,
                        );
                    }
                    "subcommands" => {
                        self.write_subcommands(self.cmd);
                    }
                    "tab" => {
                        self.writer.push_str(TAB);
                    }
                    "after-help" => {
                        self.write_after_help();
                    }
                    "before-help" => {
                        self.write_before_help();
                    }
                    _ => {
                        let _ = write!(self.writer, "{{{tag}}}");
                    }
                }
                self.writer.push_str(rest);
            }
        }
    }
}

/// Basic template methods
impl HelpTemplate<'_, '_> {
    /// Writes binary name of a Parser Object to the wrapped stream.
    fn write_display_name(&mut self) {
        debug!("HelpTemplate::write_display_name");

        let display_name = wrap(
            &self
                .cmd
                .get_display_name()
                .unwrap_or_else(|| self.cmd.get_name())
                .replace("{n}", "\n"),
            self.term_w,
        );
        self.writer.push_string(display_name);
    }

    /// Writes binary name of a Parser Object to the wrapped stream.
    #[cfg(not(feature = "unstable-v5"))]
    fn write_bin_name(&mut self) {
        debug!("HelpTemplate::write_bin_name");

        let bin_name = if let Some(bn) = self.cmd.get_bin_name() {
            if bn.contains(' ') {
                // In case we're dealing with subcommands i.e. git mv is translated to git-mv
                bn.replace(' ', "-")
            } else {
                wrap(&self.cmd.get_name().replace("{n}", "\n"), self.term_w)
            }
        } else {
            wrap(&self.cmd.get_name().replace("{n}", "\n"), self.term_w)
        };
        self.writer.push_string(bin_name);
    }

    fn write_version(&mut self) {
        let version = self
            .cmd
            .get_version()
            .or_else(|| self.cmd.get_long_version());
        if let Some(output) = version {
            self.writer.push_string(wrap(output, self.term_w));
        }
    }

    fn write_author(&mut self, before_new_line: bool, after_new_line: bool) {
        if let Some(author) = self.cmd.get_author() {
            if before_new_line {
                self.writer.push_str("\n");
            }
            self.writer.push_string(wrap(author, self.term_w));
            if after_new_line {
                self.writer.push_str("\n");
            }
        }
    }

    fn write_about(&mut self, before_new_line: bool, after_new_line: bool) {
        let about = if self.use_long {
            self.cmd.get_long_about().or_else(|| self.cmd.get_about())
        } else {
            self.cmd.get_about()
        };
        if let Some(output) = about {
            if before_new_line {
                self.writer.push_str("\n");
            }
            let mut output = output.clone();
            output.replace_newline_var();
            output.wrap(self.term_w);
            self.writer.push_styled(&output);
            if after_new_line {
                self.writer.push_str("\n");
            }
        }
    }

    fn write_before_help(&mut self) {
        debug!("HelpTemplate::write_before_help");
        let before_help = if self.use_long {
            self.cmd
                .get_before_long_help()
                .or_else(|| self.cmd.get_before_help())
        } else {
            self.cmd.get_before_help()
        };
        if let Some(output) = before_help {
            let mut output = output.clone();
            output.replace_newline_var();
            output.wrap(self.term_w);
            self.writer.push_styled(&output);
            self.writer.push_str("\n\n");
        }
    }

    fn write_after_help(&mut self) {
        debug!("HelpTemplate::write_after_help");
        let after_help = if self.use_long {
            self.cmd
                .get_after_long_help()
                .or_else(|| self.cmd.get_after_help())
        } else {
            self.cmd.get_after_help()
        };
        if let Some(output) = after_help {
            self.writer.push_str("\n\n");
            let mut output = output.clone();
            output.replace_newline_var();
            output.wrap(self.term_w);
            self.writer.push_styled(&output);
        }
    }
}

/// Arg handling
impl HelpTemplate<'_, '_> {
    /// Writes help for all arguments (options, flags, args, subcommands)
    /// including titles of a Parser Object to the wrapped stream.
    pub(crate) fn write_all_args(&mut self) {
        debug!("HelpTemplate::write_all_args");
        use std::fmt::Write as _;
        let header = &self.styles.get_header();

        let pos = self
            .cmd
            .get_positionals()
            .filter(|a| a.get_help_heading().is_none())
            .filter(|arg| should_show_arg(self.use_long, arg))
            .collect::<Vec<_>>();
        let non_pos = self
            .cmd
            .get_non_positionals()
            .filter(|a| a.get_help_heading().is_none())
            .filter(|arg| should_show_arg(self.use_long, arg))
            .collect::<Vec<_>>();
        let subcmds = self.cmd.has_visible_subcommands();

        let custom_headings = self
            .cmd
            .get_arguments()
            .filter_map(|arg| arg.get_help_heading())
            .collect::<FlatSet<_>>();

        let flatten = self.cmd.is_flatten_help_set();

        let mut first = true;

        if subcmds && !flatten {
            if !first {
                self.writer.push_str("\n\n");
            }
            first = false;
            let default_help_heading = Str::from("Commands");
            let help_heading = self
                .cmd
                .get_subcommand_help_heading()
                .unwrap_or(&default_help_heading);
            let _ = write!(self.writer, "{header}{help_heading}:{header:#}\n",);

            self.write_subcommands(self.cmd);
        }

        if !pos.is_empty() {
            if !first {
                self.writer.push_str("\n\n");
            }
            first = false;
            // Write positional args if any
            let help_heading = "Arguments";
            let _ = write!(self.writer, "{header}{help_heading}:{header:#}\n",);
            self.write_args(&pos, "Arguments", positional_sort_key);
        }

        if !non_pos.is_empty() {
            if !first {
                self.writer.push_str("\n\n");
            }
            first = false;
            let help_heading = "Options";
            let _ = write!(self.writer, "{header}{help_heading}:{header:#}\n",);
            self.write_args(&non_pos, "Options", option_sort_key);
        }
        if !custom_headings.is_empty() {
            for heading in custom_headings {
                let args = self
                    .cmd
                    .get_arguments()
                    .filter(|a| {
                        if let Some(help_heading) = a.get_help_heading() {
                            return help_heading == heading;
                        }
                        false
                    })
                    .filter(|arg| should_show_arg(self.use_long, arg))
                    .collect::<Vec<_>>();

                if !args.is_empty() {
                    if !first {
                        self.writer.push_str("\n\n");
                    }
                    first = false;
                    let _ = write!(self.writer, "{header}{heading}:{header:#}\n",);
                    self.write_args(&args, heading, option_sort_key);
                }
            }
        }
        if subcmds && flatten {
            let mut cmd = self.cmd.clone();
            cmd.build();
            self.write_flat_subcommands(&cmd, &mut first);
        }
    }

    /// Sorts arguments by length and display order and write their help to the wrapped stream.
    fn write_args(&mut self, args: &[&Arg], _category: &str, sort_key: ArgSortKey) {
        debug!("HelpTemplate::write_args {_category}");
        // The shortest an arg can legally be is 2 (i.e. '-x')
        let mut longest = 2;
        let mut ord_v = BTreeMap::new();

        // Determine the longest
        for &arg in args.iter().filter(|arg| {
            // If it's NextLineHelp we don't care to compute how long it is because it may be
            // NextLineHelp on purpose simply *because* it's so long and would throw off all other
            // args alignment
            should_show_arg(self.use_long, arg)
        }) {
            if longest_filter(arg) {
                let width = display_width(&arg.to_string());
                let actual_width = if arg.is_positional() {
                    width
                } else {
                    width + SHORT_SIZE
                };
                longest = longest.max(actual_width);
                debug!(
                    "HelpTemplate::write_args: arg={:?} longest={}",
                    arg.get_id(),
                    longest
                );
            }

            let key = (sort_key)(arg);
            ord_v.insert(key, arg);
        }

        let next_line_help = self.will_args_wrap(args, longest);

        for (i, (_, arg)) in ord_v.iter().enumerate() {
            if i != 0 {
                self.writer.push_str("\n");
                if next_line_help && self.use_long {
                    self.writer.push_str("\n");
                }
            }
            self.write_arg(arg, next_line_help, longest);
        }
    }

    /// Writes help for an argument to the wrapped stream.
    fn write_arg(&mut self, arg: &Arg, next_line_help: bool, longest: usize) {
        let spec_vals = &self.spec_vals(arg);

        self.writer.push_str(TAB);
        self.short(arg);
        self.long(arg);
        self.writer
            .push_styled(&arg.stylize_arg_suffix(self.styles, None));
        self.align_to_about(arg, next_line_help, longest);

        let about = if self.use_long {
            arg.get_long_help()
                .or_else(|| arg.get_help())
                .unwrap_or_default()
        } else {
            arg.get_help()
                .or_else(|| arg.get_long_help())
                .unwrap_or_default()
        };

        self.help(Some(arg), about, spec_vals, next_line_help, longest);
    }

    /// Writes argument's short command to the wrapped stream.
    fn short(&mut self, arg: &Arg) {
        debug!("HelpTemplate::short");
        use std::fmt::Write as _;
        let literal = &self.styles.get_literal();

        if let Some(s) = arg.get_short() {
            let _ = write!(self.writer, "{literal}-{s}{literal:#}",);
        } else if arg.get_long().is_some() {
            self.writer.push_str("    ");
        }
    }

    /// Writes argument's long command to the wrapped stream.
    fn long(&mut self, arg: &Arg) {
        debug!("HelpTemplate::long");
        use std::fmt::Write as _;
        let literal = &self.styles.get_literal();

        if let Some(long) = arg.get_long() {
            if arg.get_short().is_some() {
                self.writer.push_str(", ");
            }
            let _ = write!(self.writer, "{literal}--{long}{literal:#}",);
        }
    }

    /// Write alignment padding between arg's switches/values and its about message.
    fn align_to_about(&mut self, arg: &Arg, next_line_help: bool, longest: usize) {
        debug!(
            "HelpTemplate::align_to_about: arg={}, next_line_help={}, longest={}",
            arg.get_id(),
            next_line_help,
            longest
        );
        let padding = if self.use_long || next_line_help {
            // long help prints messages on the next line so it doesn't need to align text
            debug!("HelpTemplate::align_to_about: printing long help so skip alignment");
            0
        } else if !arg.is_positional() {
            let self_len = display_width(&arg.to_string()) + SHORT_SIZE;
            // Since we're writing spaces from the tab point we first need to know if we
            // had a long and short, or just short
            let padding = if arg.get_long().is_some() {
                // Only account 4 after the val
                TAB_WIDTH
            } else {
                // Only account for ', --' + 4 after the val
                TAB_WIDTH + 4
            };
            let spcs = longest + padding - self_len;
            debug!(
                "HelpTemplate::align_to_about: positional=false arg_len={self_len}, spaces={spcs}"
            );

            spcs
        } else {
            let self_len = display_width(&arg.to_string());
            let padding = TAB_WIDTH;
            let spcs = longest + padding - self_len;
            debug!(
                "HelpTemplate::align_to_about: positional=true arg_len={self_len}, spaces={spcs}",
            );

            spcs
        };

        self.write_padding(padding);
    }

    /// Writes argument's help to the wrapped stream.
    fn help(
        &mut self,
        arg: Option<&Arg>,
        about: &StyledStr,
        spec_vals: &str,
        next_line_help: bool,
        longest: usize,
    ) {
        debug!("HelpTemplate::help");
        use std::fmt::Write as _;
        let literal = &self.styles.get_literal();

        // Is help on next line, if so then indent
        if next_line_help {
            debug!("HelpTemplate::help: Next Line...{next_line_help:?}");
            self.writer.push_str("\n");
            self.writer.push_str(TAB);
            self.writer.push_str(NEXT_LINE_INDENT);
        }

        let spaces = if next_line_help {
            TAB.len() + NEXT_LINE_INDENT.len()
        } else {
            longest + TAB_WIDTH * 2
        };
        let trailing_indent = spaces; // Don't indent any further than the first line is indented
        let trailing_indent = self.get_spaces(trailing_indent);

        let mut help = about.clone();
        let mut help_is_empty = help.is_empty();
        help.replace_newline_var();

        let next_line_specs = self.use_long && arg.is_some();
        if !spec_vals.is_empty() && !next_line_specs {
            if !help_is_empty {
                let sep = " ";
                help.push_str(sep);
            }
            help.push_str(spec_vals);
            help_is_empty = help.is_empty();
        }

        let avail_chars = self.term_w.saturating_sub(spaces);
        debug!(
            "HelpTemplate::help: help_width={}, spaces={}, avail={}",
            spaces,
            help.display_width(),
            avail_chars
        );
        help.wrap(avail_chars);
        help.indent("", &trailing_indent);
        self.writer.push_styled(&help);

        if let Some(arg) = arg {
            if !arg.is_hide_possible_values_set() && self.use_long_pv(arg) {
                const DASH_SPACE: usize = "- ".len();
                let possible_vals = arg.get_possible_values();
                if !possible_vals.is_empty() {
                    debug!("HelpTemplate::help: Found possible vals...{possible_vals:?}");
                    let longest = possible_vals
                        .iter()
                        .filter(|f| !f.is_hide_set())
                        .map(|f| display_width(f.get_name()))
                        .max()
                        .expect("Only called with possible value");

                    let spaces = spaces + TAB_WIDTH - DASH_SPACE;
                    let trailing_indent = spaces + DASH_SPACE;
                    let trailing_indent = self.get_spaces(trailing_indent);

                    if !help_is_empty {
                        let _ = write!(self.writer, "\n\n{:spaces$}", "");
                    }
                    self.writer.push_str("Possible values:");
                    for pv in possible_vals.iter().filter(|pv| !pv.is_hide_set()) {
                        let name = pv.get_name();

                        let mut descr = StyledStr::new();
                        let _ = write!(&mut descr, "{literal}{name}{literal:#}",);
                        if let Some(help) = pv.get_help() {
                            debug!("HelpTemplate::help: Possible Value help");
                            // To align help messages
                            let padding = longest - display_width(name);
                            let _ = write!(&mut descr, ": {:padding$}", "");
                            descr.push_styled(help);
                        }

                        let avail_chars = if self.term_w > trailing_indent.len() {
                            self.term_w - trailing_indent.len()
                        } else {
                            usize::MAX
                        };
                        descr.replace_newline_var();
                        descr.wrap(avail_chars);
                        descr.indent("", &trailing_indent);

                        let _ = write!(self.writer, "\n{:spaces$}- ", "",);
                        self.writer.push_styled(&descr);
                    }
                }
            }
        }

        if !spec_vals.is_empty() && next_line_specs {
            let mut help = StyledStr::new();
            if !help_is_empty {
                let sep = "\n\n";
                help.push_str(sep);
            }
            help.push_str(spec_vals);

            help.wrap(avail_chars);
            help.indent("", &trailing_indent);
            self.writer.push_styled(&help);
        }
    }

    /// Will use next line help on writing args.
    fn will_args_wrap(&self, args: &[&Arg], longest: usize) -> bool {
        args.iter()
            .filter(|arg| should_show_arg(self.use_long, arg))
            .any(|arg| {
                let spec_vals = &self.spec_vals(arg);
                self.arg_next_line_help(arg, spec_vals, longest)
            })
    }

    fn arg_next_line_help(&self, arg: &Arg, spec_vals: &str, longest: usize) -> bool {
        if self.next_line_help || arg.is_next_line_help_set() || self.use_long {
            // setting_next_line
            true
        } else {
            // force_next_line
            let h = arg
                .get_help()
                .or_else(|| arg.get_long_help())
                .unwrap_or_default();
            let h_w = h.display_width() + display_width(spec_vals);
            let taken = longest + TAB_WIDTH * 2;
            self.term_w >= taken
                && (taken as f32 / self.term_w as f32) > 0.40
                && h_w > (self.term_w - taken)
        }
    }

    fn spec_vals(&self, a: &Arg) -> String {
        debug!("HelpTemplate::spec_vals: a={a}");
        let ctx = &self.styles.get_context();
        let ctx_val = &self.styles.get_context_value();
        let val_sep = format!("{ctx}, {ctx:#}"); // context values styled separator

        let mut spec_vals = Vec::new();
        #[cfg(feature = "env")]
        if let Some(ref env) = a.env {
            if !a.is_hide_env_set() {
                debug!(
                    "HelpTemplate::spec_vals: Found environment variable...[{:?}:{:?}]",
                    env.0, env.1
                );
                let env_val = if !a.is_hide_env_values_set() {
                    format!(
                        "={}",
                        env.1
                            .as_ref()
                            .map(|s| s.to_string_lossy())
                            .unwrap_or_default()
                    )
                } else {
                    Default::default()
                };
                let env_info = format!(
                    "{ctx}[env: {ctx:#}{ctx_val}{}{}{ctx_val:#}{ctx}]{ctx:#}",
                    env.0.to_string_lossy(),
                    env_val
                );
                spec_vals.push(env_info);
            }
        }
        if a.is_takes_value_set() && !a.is_hide_default_value_set() && !a.default_vals.is_empty() {
            debug!(
                "HelpTemplate::spec_vals: Found default value...[{:?}]",
                a.default_vals
            );

            let dvs = a
                .default_vals
                .iter()
                .map(|dv| dv.to_string_lossy())
                .map(|dv| {
                    if dv.contains(char::is_whitespace) {
                        Cow::from(format!("{dv:?}"))
                    } else {
                        dv
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            spec_vals.push(format!(
                "{ctx}[default: {ctx:#}{ctx_val}{dvs}{ctx_val:#}{ctx}]{ctx:#}"
            ));
        }

        let mut als = Vec::new();

        let short_als = a
            .short_aliases
            .iter()
            .filter(|&als| als.1) // visible
            .map(|als| format!("{ctx_val}-{}{ctx_val:#}", als.0)); // name
        debug!(
            "HelpTemplate::spec_vals: Found short aliases...{:?}",
            a.short_aliases
        );
        als.extend(short_als);

        let long_als = a
            .aliases
            .iter()
            .filter(|&als| als.1) // visible
            .map(|als| format!("{ctx_val}--{}{ctx_val:#}", als.0)); // name
        debug!("HelpTemplate::spec_vals: Found aliases...{:?}", a.aliases);
        als.extend(long_als);

        if !als.is_empty() {
            let als = als.join(&val_sep);
            spec_vals.push(format!("{ctx}[aliases: {ctx:#}{als}{ctx}]{ctx:#}"));
        }

        if !a.is_hide_possible_values_set() && !self.use_long_pv(a) {
            let possible_vals = a.get_possible_values();
            if !possible_vals.is_empty() {
                debug!("HelpTemplate::spec_vals: Found possible vals...{possible_vals:?}");

                let pvs = possible_vals
                    .iter()
                    .filter_map(PossibleValue::get_visible_quoted_name)
                    .map(|pv| format!("{ctx_val}{pv}{ctx_val:#}"))
                    .collect::<Vec<_>>()
                    .join(&val_sep);

                spec_vals.push(format!("{ctx}[possible values: {ctx:#}{pvs}{ctx}]{ctx:#}"));
            }
        }
        let connector = if self.use_long { "\n" } else { " " };
        spec_vals.join(connector)
    }

    fn get_spaces(&self, n: usize) -> String {
        " ".repeat(n)
    }

    fn write_padding(&mut self, amount: usize) {
        use std::fmt::Write as _;
        let _ = write!(self.writer, "{:amount$}", "");
    }

    fn use_long_pv(&self, arg: &Arg) -> bool {
        self.use_long
            && arg
                .get_possible_values()
                .iter()
                .any(PossibleValue::should_show_help)
    }
}

/// Subcommand handling
impl HelpTemplate<'_, '_> {
    /// Writes help for subcommands of a Parser Object to the wrapped stream.
    fn write_flat_subcommands(&mut self, cmd: &Command, first: &mut bool) {
        debug!(
            "HelpTemplate::write_flat_subcommands, cmd={}, first={}",
            cmd.get_name(),
            *first
        );
        use std::fmt::Write as _;
        let header = &self.styles.get_header();

        let mut ord_v = BTreeMap::new();
        for subcommand in cmd
            .get_subcommands()
            .filter(|subcommand| should_show_subcommand(subcommand))
        {
            ord_v.insert(
                (subcommand.get_display_order(), subcommand.get_name()),
                subcommand,
            );
        }
        for (_, subcommand) in ord_v {
            if !*first {
                self.writer.push_str("\n\n");
            }
            *first = false;

            let heading = subcommand.get_usage_name_fallback();
            let about = subcommand
                .get_about()
                .or_else(|| subcommand.get_long_about())
                .unwrap_or_default();

            let _ = write!(self.writer, "{header}{heading}:{header:#}",);
            if !about.is_empty() {
                let _ = write!(self.writer, "\n{about}",);
            }

            let args = subcommand
                .get_arguments()
                .filter(|arg| should_show_arg(self.use_long, arg) && !arg.is_global_set())
                .collect::<Vec<_>>();
            if !args.is_empty() {
                self.writer.push_str("\n");
            }

            let mut sub_help = HelpTemplate {
                writer: self.writer,
                cmd: subcommand,
                styles: self.styles,
                usage: self.usage,
                next_line_help: self.next_line_help,
                term_w: self.term_w,
                use_long: self.use_long,
            };
            sub_help.write_args(&args, heading, option_sort_key);
            if subcommand.is_flatten_help_set() {
                sub_help.write_flat_subcommands(subcommand, first);
            }
        }
    }

    /// Writes help for subcommands of a Parser Object to the wrapped stream.
    fn write_subcommands(&mut self, cmd: &Command) {
        debug!("HelpTemplate::write_subcommands");
        use std::fmt::Write as _;
        let literal = &self.styles.get_literal();

        // The shortest an arg can legally be is 2 (i.e. '-x')
        let mut longest = 2;
        let mut ord_v = BTreeMap::new();
        for subcommand in cmd
            .get_subcommands()
            .filter(|subcommand| should_show_subcommand(subcommand))
        {
            let mut styled = StyledStr::new();
            let name = subcommand.get_name();
            let _ = write!(styled, "{literal}{name}{literal:#}",);
            if let Some(short) = subcommand.get_short_flag() {
                let _ = write!(styled, ", {literal}-{short}{literal:#}",);
            }
            if let Some(long) = subcommand.get_long_flag() {
                let _ = write!(styled, ", {literal}--{long}{literal:#}",);
            }
            longest = longest.max(styled.display_width());
            ord_v.insert((subcommand.get_display_order(), styled), subcommand);
        }

        debug!("HelpTemplate::write_subcommands longest = {longest}");

        let next_line_help = self.will_subcommands_wrap(cmd.get_subcommands(), longest);

        for (i, (sc_str, sc)) in ord_v.into_iter().enumerate() {
            if 0 < i {
                self.writer.push_str("\n");
            }
            self.write_subcommand(sc_str.1, sc, next_line_help, longest);
        }
    }

    /// Will use next line help on writing subcommands.
    fn will_subcommands_wrap<'a>(
        &self,
        subcommands: impl IntoIterator<Item = &'a Command>,
        longest: usize,
    ) -> bool {
        subcommands
            .into_iter()
            .filter(|&subcommand| should_show_subcommand(subcommand))
            .any(|subcommand| {
                let spec_vals = &self.sc_spec_vals(subcommand);
                self.subcommand_next_line_help(subcommand, spec_vals, longest)
            })
    }

    fn write_subcommand(
        &mut self,
        sc_str: StyledStr,
        cmd: &Command,
        next_line_help: bool,
        longest: usize,
    ) {
        debug!("HelpTemplate::write_subcommand");

        let spec_vals = &self.sc_spec_vals(cmd);

        let about = cmd
            .get_about()
            .or_else(|| cmd.get_long_about())
            .unwrap_or_default();

        self.subcmd(sc_str, next_line_help, longest);
        self.help(None, about, spec_vals, next_line_help, longest);
    }

    fn sc_spec_vals(&self, a: &Command) -> String {
        debug!("HelpTemplate::sc_spec_vals: a={}", a.get_name());
        let ctx = &self.styles.get_context();
        let ctx_val = &self.styles.get_context_value();
        let val_sep = format!("{ctx}, {ctx:#}"); // context values styled separator
        let mut spec_vals = vec![];

        let mut short_als = a
            .get_visible_short_flag_aliases()
            .map(|s| format!("{ctx_val}-{s}{ctx_val:#}"))
            .collect::<Vec<_>>();
        let long_als = a
            .get_visible_long_flag_aliases()
            .map(|s| format!("{ctx_val}--{s}{ctx_val:#}"));
        short_als.extend(long_als);
        let als = a
            .get_visible_aliases()
            .map(|s| format!("{ctx_val}{s}{ctx_val:#}"));
        short_als.extend(als);
        let all_als = short_als.join(&val_sep);
        if !all_als.is_empty() {
            debug!(
                "HelpTemplate::spec_vals: Found aliases...{:?}",
                a.get_all_aliases().collect::<Vec<_>>()
            );
            debug!(
                "HelpTemplate::spec_vals: Found short flag aliases...{:?}",
                a.get_all_short_flag_aliases().collect::<Vec<_>>()
            );
            debug!(
                "HelpTemplate::spec_vals: Found long flag aliases...{:?}",
                a.get_all_long_flag_aliases().collect::<Vec<_>>()
            );
            spec_vals.push(format!("{ctx}[aliases: {ctx:#}{all_als}{ctx}]{ctx:#}"));
        }

        spec_vals.join(" ")
    }

    fn subcommand_next_line_help(&self, cmd: &Command, spec_vals: &str, longest: usize) -> bool {
        // Ignore `self.use_long` since subcommands are only shown as short help
        if self.next_line_help {
            // setting_next_line
            true
        } else {
            // force_next_line
            let h = cmd
                .get_about()
                .or_else(|| cmd.get_long_about())
                .unwrap_or_default();
            let h_w = h.display_width() + display_width(spec_vals);
            let taken = longest + TAB_WIDTH * 2;
            self.term_w >= taken
                && (taken as f32 / self.term_w as f32) > 0.40
                && h_w > (self.term_w - taken)
        }
    }

    /// Writes subcommand to the wrapped stream.
    fn subcmd(&mut self, sc_str: StyledStr, next_line_help: bool, longest: usize) {
        self.writer.push_str(TAB);
        self.writer.push_styled(&sc_str);
        if !next_line_help {
            let width = sc_str.display_width();
            let padding = longest + TAB_WIDTH - width;
            self.write_padding(padding);
        }
    }
}

const NEXT_LINE_INDENT: &str = "        ";

type ArgSortKey = fn(arg: &Arg) -> (usize, String);

fn positional_sort_key(arg: &Arg) -> (usize, String) {
    (arg.get_index().unwrap_or(0), String::new())
}

fn option_sort_key(arg: &Arg) -> (usize, String) {
    // Formatting key like this to ensure that:
    // 1. Argument has long flags are printed just after short flags.
    // 2. For two args both have short flags like `-c` and `-C`, the
    //    `-C` arg is printed just after the `-c` arg
    // 3. For args without short or long flag, print them at last(sorted
    //    by arg name).
    // Example order: -a, -b, -B, -s, --select-file, --select-folder, -x

    let key = if let Some(x) = arg.get_short() {
        let mut s = x.to_ascii_lowercase().to_string();
        s.push(if x.is_ascii_lowercase() { '0' } else { '1' });
        s
    } else if let Some(x) = arg.get_long() {
        x.to_string()
    } else {
        let mut s = '{'.to_string();
        s.push_str(arg.get_id().as_str());
        s
    };
    (arg.get_display_order(), key)
}

pub(crate) fn dimensions() -> (Option<usize>, Option<usize>) {
    #[cfg(not(feature = "wrap_help"))]
    return (None, None);

    #[cfg(feature = "wrap_help")]
    terminal_size::terminal_size()
        .map(|(w, h)| (Some(w.0.into()), Some(h.0.into())))
        .unwrap_or_else(|| (parse_env("COLUMNS"), parse_env("LINES")))
}

#[cfg(feature = "wrap_help")]
fn parse_env(var: &str) -> Option<usize> {
    some!(some!(std::env::var_os(var)).to_str())
        .parse::<usize>()
        .ok()
}

fn should_show_arg(use_long: bool, arg: &Arg) -> bool {
    debug!(
        "should_show_arg: use_long={:?}, arg={}",
        use_long,
        arg.get_id()
    );
    if arg.is_hide_set() {
        return false;
    }
    (!arg.is_hide_long_help_set() && use_long)
        || (!arg.is_hide_short_help_set() && !use_long)
        || arg.is_next_line_help_set()
}

fn should_show_subcommand(subcommand: &Command) -> bool {
    !subcommand.is_hide_set()
}

fn longest_filter(arg: &Arg) -> bool {
    arg.is_takes_value_set() || arg.get_long().is_some() || arg.get_short().is_none()
}

#[cfg(test)]
mod test {
    #[test]
    #[cfg(feature = "wrap_help")]
    fn wrap_help_last_word() {
        use super::*;

        let help = String::from("foo bar baz");
        assert_eq!(wrap(&help, 5), "foo\nbar\nbaz");
    }

    #[test]
    #[cfg(feature = "unicode")]
    fn display_width_handles_non_ascii() {
        use super::*;

        // Popular Danish tongue-twister, the name of a fruit dessert.
        let text = "rødgrød med fløde";
        assert_eq!(display_width(text), 17);
        // Note that the string width is smaller than the string
        // length. This is due to the precomposed non-ASCII letters:
        assert_eq!(text.len(), 20);
    }

    #[test]
    #[cfg(feature = "unicode")]
    fn display_width_handles_emojis() {
        use super::*;

        let text = "😂";
        // There is a single `char`...
        assert_eq!(text.chars().count(), 1);
        // but it is double-width:
        assert_eq!(display_width(text), 2);
        // This is much less than the byte length:
        assert_eq!(text.len(), 4);
    }
}
