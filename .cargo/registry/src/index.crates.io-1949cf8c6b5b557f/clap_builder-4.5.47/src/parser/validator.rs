// Internal
use crate::builder::StyledStr;
use crate::builder::{Arg, ArgGroup, ArgPredicate, Command, PossibleValue};
use crate::error::{Error, Result as ClapResult};
use crate::output::Usage;
use crate::parser::ArgMatcher;
use crate::util::ChildGraph;
use crate::util::FlatMap;
use crate::util::FlatSet;
use crate::util::Id;
use crate::INTERNAL_ERROR_MSG;

pub(crate) struct Validator<'cmd> {
    cmd: &'cmd Command,
    required: ChildGraph<Id>,
}

impl<'cmd> Validator<'cmd> {
    pub(crate) fn new(cmd: &'cmd Command) -> Self {
        let required = cmd.required_graph();
        Validator { cmd, required }
    }

    pub(crate) fn validate(&mut self, matcher: &mut ArgMatcher) -> ClapResult<()> {
        debug!("Validator::validate");
        let conflicts = Conflicts::with_args(self.cmd, matcher);
        let has_subcmd = matcher.subcommand_name().is_some();

        if !has_subcmd && self.cmd.is_arg_required_else_help_set() {
            let num_user_values = matcher
                .args()
                .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
                .count();
            if num_user_values == 0 {
                let message = self.cmd.write_help_err(false);
                return Err(Error::display_help_error(self.cmd, message));
            }
        }
        if !has_subcmd && self.cmd.is_subcommand_required_set() {
            let bn = self.cmd.get_bin_name_fallback();
            return Err(Error::missing_subcommand(
                self.cmd,
                bn.to_string(),
                self.cmd
                    .all_subcommand_names()
                    .map(|s| s.to_owned())
                    .collect::<Vec<_>>(),
                Usage::new(self.cmd)
                    .required(&self.required)
                    .create_usage_with_title(&[]),
            ));
        }

        ok!(self.validate_conflicts(matcher, &conflicts));
        if !(self.cmd.is_subcommand_negates_reqs_set() && has_subcmd) {
            ok!(self.validate_required(matcher, &conflicts));
        }

        Ok(())
    }

    fn validate_conflicts(
        &mut self,
        matcher: &ArgMatcher,
        conflicts: &Conflicts,
    ) -> ClapResult<()> {
        debug!("Validator::validate_conflicts");

        ok!(self.validate_exclusive(matcher));

        for (arg_id, _) in matcher
            .args()
            .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
            .filter(|(arg_id, _)| self.cmd.find(arg_id).is_some())
        {
            debug!("Validator::validate_conflicts::iter: id={arg_id:?}");
            let conflicts = conflicts.gather_conflicts(self.cmd, arg_id);
            ok!(self.build_conflict_err(arg_id, &conflicts, matcher));
        }

        Ok(())
    }

    fn validate_exclusive(&self, matcher: &ArgMatcher) -> ClapResult<()> {
        debug!("Validator::validate_exclusive");
        let args_count = matcher
            .args()
            .filter(|(arg_id, matched)| {
                matched.check_explicit(&ArgPredicate::IsPresent)
                    // Avoid including our own groups by checking none of them.  If a group is present, the
                    // args for the group will be.
                    && self.cmd.find(arg_id).is_some()
            })
            .count();
        if args_count <= 1 {
            // Nothing present to conflict with
            return Ok(());
        }

        matcher
            .args()
            .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
            .find_map(|(id, _)| {
                debug!("Validator::validate_exclusive:iter:{id:?}");
                self.cmd
                    .find(id)
                    // Find `arg`s which are exclusive but also appear with other args.
                    .filter(|&arg| arg.is_exclusive_set() && args_count > 1)
            })
            .map(|arg| {
                // Throw an error for the first conflict found.
                Err(Error::argument_conflict(
                    self.cmd,
                    arg.to_string(),
                    Vec::new(),
                    Usage::new(self.cmd)
                        .required(&self.required)
                        .create_usage_with_title(&[]),
                ))
            })
            .unwrap_or(Ok(()))
    }

    fn build_conflict_err(
        &self,
        name: &Id,
        conflict_ids: &[Id],
        matcher: &ArgMatcher,
    ) -> ClapResult<()> {
        if conflict_ids.is_empty() {
            return Ok(());
        }

        debug!("Validator::build_conflict_err: name={name:?}");
        let mut seen = FlatSet::new();
        let conflicts = conflict_ids
            .iter()
            .flat_map(|c_id| {
                if self.cmd.find_group(c_id).is_some() {
                    self.cmd.unroll_args_in_group(c_id)
                } else {
                    vec![c_id.clone()]
                }
            })
            .filter_map(|c_id| {
                seen.insert(c_id.clone()).then(|| {
                    let c_arg = self.cmd.find(&c_id).expect(INTERNAL_ERROR_MSG);
                    c_arg.to_string()
                })
            })
            .collect();

        let former_arg = self.cmd.find(name).expect(INTERNAL_ERROR_MSG);
        let usg = self.build_conflict_err_usage(matcher, conflict_ids);
        Err(Error::argument_conflict(
            self.cmd,
            former_arg.to_string(),
            conflicts,
            usg,
        ))
    }

    fn build_conflict_err_usage(
        &self,
        matcher: &ArgMatcher,
        conflicting_keys: &[Id],
    ) -> Option<StyledStr> {
        let used_filtered: Vec<Id> = matcher
            .args()
            .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
            .map(|(n, _)| n)
            .filter(|n| {
                // Filter out the args we don't want to specify.
                self.cmd
                    .find(n)
                    .map(|a| !a.is_hide_set())
                    .unwrap_or_default()
            })
            .filter(|key| !conflicting_keys.contains(key))
            .cloned()
            .collect();
        let required: Vec<Id> = used_filtered
            .iter()
            .filter_map(|key| self.cmd.find(key))
            .flat_map(|arg| arg.requires.iter().map(|item| &item.1))
            .filter(|key| !used_filtered.contains(key) && !conflicting_keys.contains(key))
            .chain(used_filtered.iter())
            .cloned()
            .collect();
        Usage::new(self.cmd)
            .required(&self.required)
            .create_usage_with_title(&required)
    }

    fn gather_requires(&mut self, matcher: &ArgMatcher) {
        debug!("Validator::gather_requires");
        for (name, matched) in matcher
            .args()
            .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
        {
            debug!("Validator::gather_requires:iter:{name:?}");
            if let Some(arg) = self.cmd.find(name) {
                let is_relevant = |(val, req_arg): &(ArgPredicate, Id)| -> Option<Id> {
                    let required = matched.check_explicit(val);
                    required.then(|| req_arg.clone())
                };

                for req in self.cmd.unroll_arg_requires(is_relevant, arg.get_id()) {
                    self.required.insert(req);
                }
            } else if let Some(g) = self.cmd.find_group(name) {
                debug!("Validator::gather_requires:iter:{name:?}:group");
                for r in &g.requires {
                    self.required.insert(r.clone());
                }
            }
        }
    }

    fn validate_required(&mut self, matcher: &ArgMatcher, conflicts: &Conflicts) -> ClapResult<()> {
        debug!("Validator::validate_required: required={:?}", self.required);
        self.gather_requires(matcher);

        let mut missing_required = Vec::new();
        let mut highest_index = 0;

        let is_exclusive_present = matcher
            .args()
            .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
            .any(|(id, _)| {
                self.cmd
                    .find(id)
                    .map(|arg| arg.is_exclusive_set())
                    .unwrap_or_default()
            });
        debug!("Validator::validate_required: is_exclusive_present={is_exclusive_present}");

        for arg_or_group in self
            .required
            .iter()
            .filter(|r| !matcher.check_explicit(r, &ArgPredicate::IsPresent))
        {
            debug!("Validator::validate_required:iter:aog={arg_or_group:?}");
            if let Some(arg) = self.cmd.find(arg_or_group) {
                debug!("Validator::validate_required:iter: This is an arg");
                if !is_exclusive_present && !self.is_missing_required_ok(arg, conflicts) {
                    debug!(
                        "Validator::validate_required:iter: Missing {:?}",
                        arg.get_id()
                    );
                    missing_required.push(arg.get_id().clone());
                    if !arg.is_last_set() {
                        highest_index = highest_index.max(arg.get_index().unwrap_or(0));
                    }
                }
            } else if let Some(group) = self.cmd.find_group(arg_or_group) {
                debug!("Validator::validate_required:iter: This is a group");
                if !self
                    .cmd
                    .unroll_args_in_group(&group.id)
                    .iter()
                    .any(|a| matcher.check_explicit(a, &ArgPredicate::IsPresent))
                {
                    debug!(
                        "Validator::validate_required:iter: Missing {:?}",
                        group.get_id()
                    );
                    missing_required.push(group.get_id().clone());
                }
            }
        }

        // Validate the conditionally required args
        for a in self
            .cmd
            .get_arguments()
            .filter(|a| !matcher.check_explicit(a.get_id(), &ArgPredicate::IsPresent))
        {
            let mut required = false;

            for (other, val) in &a.r_ifs {
                if matcher.check_explicit(other, &ArgPredicate::Equals(val.into())) {
                    debug!(
                        "Validator::validate_required:iter: Missing {:?}",
                        a.get_id()
                    );
                    required = true;
                }
            }

            let match_all = a.r_ifs_all.iter().all(|(other, val)| {
                matcher.check_explicit(other, &ArgPredicate::Equals(val.into()))
            });
            if match_all && !a.r_ifs_all.is_empty() {
                debug!(
                    "Validator::validate_required:iter: Missing {:?}",
                    a.get_id()
                );
                required = true;
            }

            if (!a.r_unless.is_empty() || !a.r_unless_all.is_empty())
                && self.fails_arg_required_unless(a, matcher)
            {
                debug!(
                    "Validator::validate_required:iter: Missing {:?}",
                    a.get_id()
                );
                required = true;
            }

            if !is_exclusive_present && required {
                missing_required.push(a.get_id().clone());
                if !a.is_last_set() {
                    highest_index = highest_index.max(a.get_index().unwrap_or(0));
                }
            }
        }

        // For display purposes, include all of the preceding positional arguments
        if !self.cmd.is_allow_missing_positional_set() {
            for pos in self
                .cmd
                .get_positionals()
                .filter(|a| !matcher.check_explicit(a.get_id(), &ArgPredicate::IsPresent))
            {
                if pos.get_index() < Some(highest_index) {
                    debug!(
                        "Validator::validate_required:iter: Missing {:?}",
                        pos.get_id()
                    );
                    missing_required.push(pos.get_id().clone());
                }
            }
        }

        if !missing_required.is_empty() {
            ok!(self.missing_required_error(matcher, missing_required));
        }

        Ok(())
    }

    fn is_missing_required_ok(&self, a: &Arg, conflicts: &Conflicts) -> bool {
        debug!("Validator::is_missing_required_ok: {}", a.get_id());
        if !conflicts.gather_conflicts(self.cmd, a.get_id()).is_empty() {
            debug!("Validator::is_missing_required_ok: true (self)");
            return true;
        }
        for group_id in self.cmd.groups_for_arg(a.get_id()) {
            if !conflicts.gather_conflicts(self.cmd, &group_id).is_empty() {
                debug!("Validator::is_missing_required_ok: true ({group_id})");
                return true;
            }
        }
        false
    }

    // Failing a required unless means, the arg's "unless" wasn't present, and neither were they
    fn fails_arg_required_unless(&self, a: &Arg, matcher: &ArgMatcher) -> bool {
        debug!("Validator::fails_arg_required_unless: a={:?}", a.get_id());
        let exists = |id| matcher.check_explicit(id, &ArgPredicate::IsPresent);

        (a.r_unless_all.is_empty() || !a.r_unless_all.iter().all(exists))
            && !a.r_unless.iter().any(exists)
    }

    // `req_args`: an arg to include in the error even if not used
    fn missing_required_error(
        &self,
        matcher: &ArgMatcher,
        raw_req_args: Vec<Id>,
    ) -> ClapResult<()> {
        debug!("Validator::missing_required_error; incl={raw_req_args:?}");
        debug!(
            "Validator::missing_required_error: reqs={:?}",
            self.required
        );

        let usg = Usage::new(self.cmd).required(&self.required);

        let req_args = {
            #[cfg(feature = "usage")]
            {
                usg.get_required_usage_from(&raw_req_args, Some(matcher), true)
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            }

            #[cfg(not(feature = "usage"))]
            {
                raw_req_args
                    .iter()
                    .map(|id| {
                        if let Some(arg) = self.cmd.find(id) {
                            arg.to_string()
                        } else if let Some(_group) = self.cmd.find_group(id) {
                            self.cmd.format_group(id).to_string()
                        } else {
                            debug_assert!(false, "id={id:?} is unknown");
                            "".to_owned()
                        }
                    })
                    .collect::<FlatSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            }
        };

        debug!("Validator::missing_required_error: req_args={req_args:#?}");

        let used: Vec<Id> = matcher
            .args()
            .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
            .map(|(n, _)| n)
            .filter(|n| {
                // Filter out the args we don't want to specify.
                self.cmd
                    .find(n)
                    .map(|a| !a.is_hide_set())
                    .unwrap_or_default()
            })
            .cloned()
            .chain(raw_req_args)
            .collect();

        Err(Error::missing_required_argument(
            self.cmd,
            req_args,
            usg.create_usage_with_title(&used),
        ))
    }
}

#[derive(Default, Clone, Debug)]
struct Conflicts {
    potential: FlatMap<Id, Vec<Id>>,
}

impl Conflicts {
    fn with_args(cmd: &Command, matcher: &ArgMatcher) -> Self {
        let mut potential = FlatMap::new();
        potential.extend_unchecked(
            matcher
                .args()
                .filter(|(_, matched)| matched.check_explicit(&ArgPredicate::IsPresent))
                .map(|(id, _)| {
                    let conf = gather_direct_conflicts(cmd, id);
                    (id.clone(), conf)
                }),
        );
        Self { potential }
    }

    fn gather_conflicts(&self, cmd: &Command, arg_id: &Id) -> Vec<Id> {
        debug!("Conflicts::gather_conflicts: arg={arg_id:?}");
        let mut conflicts = Vec::new();

        let arg_id_conflicts_storage;
        let arg_id_conflicts = if let Some(arg_id_conflicts) = self.get_direct_conflicts(arg_id) {
            arg_id_conflicts
        } else {
            // `is_missing_required_ok` is a case where we check not-present args for conflicts
            arg_id_conflicts_storage = gather_direct_conflicts(cmd, arg_id);
            &arg_id_conflicts_storage
        };
        for (other_arg_id, other_arg_id_conflicts) in self.potential.iter() {
            if arg_id == other_arg_id {
                continue;
            }

            if arg_id_conflicts.contains(other_arg_id) {
                conflicts.push(other_arg_id.clone());
            }
            if other_arg_id_conflicts.contains(arg_id) {
                conflicts.push(other_arg_id.clone());
            }
        }

        debug!("Conflicts::gather_conflicts: conflicts={conflicts:?}");
        conflicts
    }

    fn get_direct_conflicts(&self, arg_id: &Id) -> Option<&[Id]> {
        self.potential.get(arg_id).map(Vec::as_slice)
    }
}

fn gather_direct_conflicts(cmd: &Command, id: &Id) -> Vec<Id> {
    let conf = if let Some(arg) = cmd.find(id) {
        gather_arg_direct_conflicts(cmd, arg)
    } else if let Some(group) = cmd.find_group(id) {
        gather_group_direct_conflicts(group)
    } else {
        debug_assert!(false, "id={id:?} is unknown");
        Vec::new()
    };
    debug!("Conflicts::gather_direct_conflicts id={id:?}, conflicts={conf:?}",);
    conf
}

fn gather_arg_direct_conflicts(cmd: &Command, arg: &Arg) -> Vec<Id> {
    let mut conf = arg.blacklist.clone();
    for group_id in cmd.groups_for_arg(arg.get_id()) {
        let group = cmd.find_group(&group_id).expect(INTERNAL_ERROR_MSG);
        conf.extend(group.conflicts.iter().cloned());
        if !group.multiple {
            for member_id in &group.args {
                if member_id != arg.get_id() {
                    conf.push(member_id.clone());
                }
            }
        }
    }

    // Overrides are implicitly conflicts
    conf.extend(arg.overrides.iter().cloned());

    conf
}

fn gather_group_direct_conflicts(group: &ArgGroup) -> Vec<Id> {
    group.conflicts.clone()
}

pub(crate) fn get_possible_values_cli(a: &Arg) -> Vec<PossibleValue> {
    if !a.is_takes_value_set() {
        vec![]
    } else {
        a.get_value_parser()
            .possible_values()
            .map(|pvs| pvs.collect())
            .unwrap_or_default()
    }
}
