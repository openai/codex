#![cfg_attr(not(feature = "usage"), allow(unused_imports))]
#![cfg_attr(not(feature = "usage"), allow(unused_variables))]
#![cfg_attr(not(feature = "usage"), allow(clippy::manual_map))]
#![cfg_attr(not(feature = "usage"), allow(dead_code))]

// Internal
use crate::builder::ArgAction;
use crate::builder::StyledStr;
use crate::builder::Styles;
use crate::builder::{ArgPredicate, Command};
use crate::parser::ArgMatcher;
use crate::util::ChildGraph;
use crate::util::FlatSet;
use crate::util::Id;

static DEFAULT_SUB_VALUE_NAME: &str = "COMMAND";
const USAGE_SEP: &str = "\n       ";

pub(crate) struct Usage<'cmd> {
    cmd: &'cmd Command,
    styles: &'cmd Styles,
    required: Option<&'cmd ChildGraph<Id>>,
}

impl<'cmd> Usage<'cmd> {
    pub(crate) fn new(cmd: &'cmd Command) -> Self {
        Usage {
            cmd,
            styles: cmd.get_styles(),
            required: None,
        }
    }

    pub(crate) fn required(mut self, required: &'cmd ChildGraph<Id>) -> Self {
        self.required = Some(required);
        self
    }

    // Creates a usage string for display. This happens just after all arguments were parsed, but before
    // any subcommands have been parsed (so as to give subcommands their own usage recursively)
    pub(crate) fn create_usage_with_title(&self, used: &[Id]) -> Option<StyledStr> {
        debug!("Usage::create_usage_with_title");
        use std::fmt::Write as _;
        let mut styled = StyledStr::new();
        let _ = write!(
            styled,
            "{}Usage:{} ",
            self.styles.get_usage().render(),
            self.styles.get_usage().render_reset()
        );
        if self.write_usage_no_title(&mut styled, used) {
            styled.trim_end();
        } else {
            return None;
        }
        debug!("Usage::create_usage_with_title: usage={styled}");
        Some(styled)
    }

    // Creates a usage string (*without title*) if one was not provided by the user manually.
    pub(crate) fn create_usage_no_title(&self, used: &[Id]) -> Option<StyledStr> {
        debug!("Usage::create_usage_no_title");

        let mut styled = StyledStr::new();
        if self.write_usage_no_title(&mut styled, used) {
            styled.trim_end();
            debug!("Usage::create_usage_no_title: usage={styled}");
            Some(styled)
        } else {
            None
        }
    }

    // Creates a usage string (*without title*) if one was not provided by the user manually.
    fn write_usage_no_title(&self, styled: &mut StyledStr, used: &[Id]) -> bool {
        debug!("Usage::create_usage_no_title");
        if let Some(u) = self.cmd.get_override_usage() {
            styled.push_styled(u);
            true
        } else {
            #[cfg(feature = "usage")]
            {
                if used.is_empty() {
                    self.write_help_usage(styled);
                } else {
                    self.write_smart_usage(styled, used);
                }
                true
            }

            #[cfg(not(feature = "usage"))]
            {
                false
            }
        }
    }
}

#[cfg(feature = "usage")]
impl Usage<'_> {
    // Creates a usage string for display in help messages (i.e. not for errors)
    fn write_help_usage(&self, styled: &mut StyledStr) {
        debug!("Usage::write_help_usage");
        use std::fmt::Write;

        if self.cmd.has_visible_subcommands() && self.cmd.is_flatten_help_set() {
            if !self.cmd.is_subcommand_required_set()
                || self.cmd.is_args_conflicts_with_subcommands_set()
            {
                self.write_arg_usage(styled, &[], true);
                styled.trim_end();
                let _ = write!(styled, "{USAGE_SEP}");
            }
            let mut cmd = self.cmd.clone();
            cmd.build();
            for (i, sub) in cmd
                .get_subcommands()
                .filter(|c| !c.is_hide_set())
                .enumerate()
            {
                if i != 0 {
                    styled.trim_end();
                    let _ = write!(styled, "{USAGE_SEP}");
                }
                Usage::new(sub).write_usage_no_title(styled, &[]);
            }
        } else {
            self.write_arg_usage(styled, &[], true);
            self.write_subcommand_usage(styled);
        }
    }

    // Creates a context aware usage string, or "smart usage" from currently used
    // args, and requirements
    fn write_smart_usage(&self, styled: &mut StyledStr, used: &[Id]) {
        debug!("Usage::create_smart_usage");
        use std::fmt::Write;
        let placeholder = &self.styles.get_placeholder();

        self.write_arg_usage(styled, used, true);

        if self.cmd.is_subcommand_required_set() {
            let value_name = self
                .cmd
                .get_subcommand_value_name()
                .unwrap_or(DEFAULT_SUB_VALUE_NAME);
            let _ = write!(styled, "{placeholder}<{value_name}>{placeholder:#}",);
        }
    }

    fn write_arg_usage(&self, styled: &mut StyledStr, used: &[Id], incl_reqs: bool) {
        debug!("Usage::write_arg_usage; incl_reqs={incl_reqs:?}");
        use std::fmt::Write as _;
        let literal = &self.styles.get_literal();
        let placeholder = &self.styles.get_placeholder();

        let bin_name = self.cmd.get_usage_name_fallback();
        if !bin_name.is_empty() {
            // the trim won't properly remove a leading space due to the formatting
            let _ = write!(styled, "{literal}{bin_name}{literal:#} ",);
        }

        if used.is_empty() && self.needs_options_tag() {
            let _ = write!(styled, "{placeholder}[OPTIONS]{placeholder:#} ",);
        }

        self.write_args(styled, used, !incl_reqs);
    }

    fn write_subcommand_usage(&self, styled: &mut StyledStr) {
        debug!("Usage::write_subcommand_usage");
        use std::fmt::Write as _;

        // incl_reqs is only false when this function is called recursively
        if self.cmd.has_visible_subcommands() || self.cmd.is_allow_external_subcommands_set() {
            let literal = &self.styles.get_literal();
            let placeholder = &self.styles.get_placeholder();
            let value_name = self
                .cmd
                .get_subcommand_value_name()
                .unwrap_or(DEFAULT_SUB_VALUE_NAME);
            if self.cmd.is_subcommand_negates_reqs_set()
                || self.cmd.is_args_conflicts_with_subcommands_set()
            {
                styled.trim_end();
                let _ = write!(styled, "{USAGE_SEP}");
                if self.cmd.is_args_conflicts_with_subcommands_set() {
                    let bin_name = self.cmd.get_usage_name_fallback();
                    // Short-circuit full usage creation since no args will be relevant
                    let _ = write!(styled, "{literal}{bin_name}{literal:#} ",);
                } else {
                    self.write_arg_usage(styled, &[], false);
                }
                let _ = write!(styled, "{placeholder}<{value_name}>{placeholder:#}",);
            } else if self.cmd.is_subcommand_required_set() {
                let _ = write!(styled, "{placeholder}<{value_name}>{placeholder:#}",);
            } else {
                let _ = write!(styled, "{placeholder}[{value_name}]{placeholder:#}",);
            }
        }
    }

    // Determines if we need the `[OPTIONS]` tag in the usage string
    fn needs_options_tag(&self) -> bool {
        debug!("Usage::needs_options_tag");
        'outer: for f in self.cmd.get_non_positionals() {
            debug!("Usage::needs_options_tag:iter: f={}", f.get_id());

            // Don't print `[OPTIONS]` just for help or version
            if f.get_long() == Some("help") || f.get_long() == Some("version") {
                debug!("Usage::needs_options_tag:iter Option is built-in");
                continue;
            }
            match f.get_action() {
                ArgAction::Set
                | ArgAction::Append
                | ArgAction::SetTrue
                | ArgAction::SetFalse
                | ArgAction::Count => {}
                ArgAction::Help
                | ArgAction::HelpShort
                | ArgAction::HelpLong
                | ArgAction::Version => {
                    debug!("Usage::needs_options_tag:iter Option is built-in");
                    continue;
                }
            }

            if f.is_hide_set() {
                debug!("Usage::needs_options_tag:iter Option is hidden");
                continue;
            }
            if f.is_required_set() {
                debug!("Usage::needs_options_tag:iter Option is required");
                continue;
            }
            for grp_s in self.cmd.groups_for_arg(f.get_id()) {
                debug!("Usage::needs_options_tag:iter:iter: grp_s={grp_s:?}");
                if self.cmd.get_groups().any(|g| g.id == grp_s && g.required) {
                    debug!("Usage::needs_options_tag:iter:iter: Group is required");
                    continue 'outer;
                }
            }

            debug!("Usage::needs_options_tag:iter: [OPTIONS] required");
            return true;
        }

        debug!("Usage::needs_options_tag: [OPTIONS] not required");
        false
    }

    // Returns the required args in usage string form by fully unrolling all groups
    pub(crate) fn write_args(&self, styled: &mut StyledStr, incls: &[Id], force_optional: bool) {
        debug!("Usage::write_args: incls={incls:?}",);
        use std::fmt::Write as _;
        let literal = &self.styles.get_literal();

        let required_owned;
        let required = if let Some(required) = self.required {
            required
        } else {
            required_owned = self.cmd.required_graph();
            &required_owned
        };

        let mut unrolled_reqs = Vec::new();
        for a in required.iter() {
            let is_relevant = |(val, req_arg): &(ArgPredicate, Id)| -> Option<Id> {
                let required = match val {
                    ArgPredicate::Equals(_) => false,
                    ArgPredicate::IsPresent => true,
                };
                required.then(|| req_arg.clone())
            };

            for aa in self.cmd.unroll_arg_requires(is_relevant, a) {
                // if we don't check for duplicates here this causes duplicate error messages
                // see https://github.com/clap-rs/clap/issues/2770
                unrolled_reqs.push(aa);
            }
            // always include the required arg itself. it will not be enumerated
            // by unroll_requirements_for_arg.
            unrolled_reqs.push(a.clone());
        }
        debug!("Usage::get_args: unrolled_reqs={unrolled_reqs:?}");

        let mut required_groups_members = FlatSet::new();
        let mut required_groups = FlatSet::new();
        for req in unrolled_reqs.iter().chain(incls.iter()) {
            if self.cmd.find_group(req).is_some() {
                let group_members = self.cmd.unroll_args_in_group(req);
                let elem = self.cmd.format_group(req);
                required_groups.insert(elem);
                required_groups_members.extend(group_members);
            } else {
                debug_assert!(self.cmd.find(req).is_some());
            }
        }

        let mut required_opts = FlatSet::new();
        let mut required_positionals = Vec::new();
        for req in unrolled_reqs.iter().chain(incls.iter()) {
            if let Some(arg) = self.cmd.find(req) {
                if required_groups_members.contains(arg.get_id()) {
                    continue;
                }

                let stylized = arg.stylized(self.styles, Some(!force_optional));
                if let Some(index) = arg.get_index() {
                    let new_len = index + 1;
                    if required_positionals.len() < new_len {
                        required_positionals.resize(new_len, None);
                    }
                    required_positionals[index] = Some(stylized);
                } else {
                    required_opts.insert(stylized);
                }
            } else {
                debug_assert!(self.cmd.find_group(req).is_some());
            }
        }

        for pos in self.cmd.get_positionals() {
            if pos.is_hide_set() {
                continue;
            }
            if required_groups_members.contains(pos.get_id()) {
                continue;
            }

            let index = pos.get_index().unwrap();
            let new_len = index + 1;
            if required_positionals.len() < new_len {
                required_positionals.resize(new_len, None);
            }
            if required_positionals[index].is_some() {
                if pos.is_last_set() {
                    let styled = required_positionals[index].take().unwrap();
                    let mut new = StyledStr::new();
                    let _ = write!(new, "{literal}--{literal:#} ");
                    new.push_styled(&styled);
                    required_positionals[index] = Some(new);
                }
            } else {
                let mut styled;
                if pos.is_last_set() {
                    styled = StyledStr::new();
                    let _ = write!(styled, "{literal}[--{literal:#} ");
                    styled.push_styled(&pos.stylized(self.styles, Some(true)));
                    let _ = write!(styled, "{literal}]{literal:#}");
                } else {
                    styled = pos.stylized(self.styles, Some(false));
                }
                required_positionals[index] = Some(styled);
            }
            if pos.is_last_set() && force_optional {
                required_positionals[index] = None;
            }
        }

        if !force_optional {
            for arg in required_opts {
                styled.push_styled(&arg);
                styled.push_str(" ");
            }
            for arg in required_groups {
                styled.push_styled(&arg);
                styled.push_str(" ");
            }
        }
        for arg in required_positionals.into_iter().flatten() {
            styled.push_styled(&arg);
            styled.push_str(" ");
        }
    }

    pub(crate) fn get_required_usage_from(
        &self,
        incls: &[Id],
        matcher: Option<&ArgMatcher>,
        incl_last: bool,
    ) -> Vec<StyledStr> {
        debug!(
            "Usage::get_required_usage_from: incls={:?}, matcher={:?}, incl_last={:?}",
            incls,
            matcher.is_some(),
            incl_last
        );

        let required_owned;
        let required = if let Some(required) = self.required {
            required
        } else {
            required_owned = self.cmd.required_graph();
            &required_owned
        };

        let mut unrolled_reqs = Vec::new();
        for a in required.iter() {
            let is_relevant = |(val, req_arg): &(ArgPredicate, Id)| -> Option<Id> {
                let required = match val {
                    ArgPredicate::Equals(_) => {
                        if let Some(matcher) = matcher {
                            matcher.check_explicit(a, val)
                        } else {
                            false
                        }
                    }
                    ArgPredicate::IsPresent => true,
                };
                required.then(|| req_arg.clone())
            };

            for aa in self.cmd.unroll_arg_requires(is_relevant, a) {
                // if we don't check for duplicates here this causes duplicate error messages
                // see https://github.com/clap-rs/clap/issues/2770
                unrolled_reqs.push(aa);
            }
            // always include the required arg itself. it will not be enumerated
            // by unroll_requirements_for_arg.
            unrolled_reqs.push(a.clone());
        }
        debug!("Usage::get_required_usage_from: unrolled_reqs={unrolled_reqs:?}");

        let mut required_groups_members = FlatSet::new();
        let mut required_groups = FlatSet::new();
        for req in unrolled_reqs.iter().chain(incls.iter()) {
            if self.cmd.find_group(req).is_some() {
                let group_members = self.cmd.unroll_args_in_group(req);
                let is_present = matcher
                    .map(|m| {
                        group_members
                            .iter()
                            .any(|arg| m.check_explicit(arg, &ArgPredicate::IsPresent))
                    })
                    .unwrap_or(false);
                debug!("Usage::get_required_usage_from:iter:{req:?} group is_present={is_present}");
                if is_present {
                    continue;
                }

                let elem = self.cmd.format_group(req);
                required_groups.insert(elem);
                required_groups_members.extend(group_members);
            } else {
                debug_assert!(self.cmd.find(req).is_some(), "`{req}` must exist");
            }
        }

        let mut required_opts = FlatSet::new();
        let mut required_positionals = Vec::new();
        for req in unrolled_reqs.iter().chain(incls.iter()) {
            if let Some(arg) = self.cmd.find(req) {
                if required_groups_members.contains(arg.get_id()) {
                    continue;
                }

                let is_present = matcher
                    .map(|m| m.check_explicit(req, &ArgPredicate::IsPresent))
                    .unwrap_or(false);
                debug!("Usage::get_required_usage_from:iter:{req:?} arg is_present={is_present}");
                if is_present {
                    continue;
                }

                let stylized = arg.stylized(self.styles, Some(true));
                if let Some(index) = arg.get_index() {
                    if !arg.is_last_set() || incl_last {
                        let new_len = index + 1;
                        if required_positionals.len() < new_len {
                            required_positionals.resize(new_len, None);
                        }
                        required_positionals[index] = Some(stylized);
                    }
                } else {
                    required_opts.insert(stylized);
                }
            } else {
                debug_assert!(self.cmd.find_group(req).is_some());
            }
        }

        let mut ret_val = Vec::new();
        ret_val.extend(required_opts);
        ret_val.extend(required_groups);
        for pos in required_positionals.into_iter().flatten() {
            ret_val.push(pos);
        }

        debug!("Usage::get_required_usage_from: ret_val={ret_val:?}");
        ret_val
    }
}
