// Std
use std::ffi::OsString;
use std::mem;
use std::ops::Deref;

// Internal
use crate::builder::{Arg, ArgPredicate, Command};
use crate::parser::Identifier;
use crate::parser::PendingArg;
use crate::parser::{ArgMatches, MatchedArg, SubCommand, ValueSource};
use crate::util::AnyValue;
use crate::util::FlatMap;
use crate::util::Id;
use crate::INTERNAL_ERROR_MSG;

#[derive(Debug, Default)]
pub(crate) struct ArgMatcher {
    matches: ArgMatches,
    pending: Option<PendingArg>,
}

impl ArgMatcher {
    pub(crate) fn new(_cmd: &Command) -> Self {
        ArgMatcher {
            matches: ArgMatches {
                #[cfg(debug_assertions)]
                valid_args: {
                    let args = _cmd.get_arguments().map(|a| a.get_id().clone());
                    let groups = _cmd.get_groups().map(|g| g.get_id().clone());
                    args.chain(groups).collect()
                },
                #[cfg(debug_assertions)]
                valid_subcommands: _cmd
                    .get_subcommands()
                    .map(|sc| sc.get_name_str().clone())
                    .collect(),
                ..Default::default()
            },
            pending: None,
        }
    }

    pub(crate) fn into_inner(self) -> ArgMatches {
        self.matches
    }

    pub(crate) fn propagate_globals(&mut self, global_arg_vec: &[Id]) {
        debug!("ArgMatcher::get_global_values: global_arg_vec={global_arg_vec:?}");
        let mut vals_map = FlatMap::new();
        self.fill_in_global_values(global_arg_vec, &mut vals_map);
    }

    fn fill_in_global_values(
        &mut self,
        global_arg_vec: &[Id],
        vals_map: &mut FlatMap<Id, MatchedArg>,
    ) {
        for global_arg in global_arg_vec {
            if let Some(ma) = self.get(global_arg) {
                // We have to check if the parent's global arg wasn't used but still exists
                // such as from a default value.
                //
                // For example, `myprog subcommand --global-arg=value` where `--global-arg` defines
                // a default value of `other` myprog would have an existing MatchedArg for
                // `--global-arg` where the value is `other`
                let to_update = if let Some(parent_ma) = vals_map.get(global_arg) {
                    if parent_ma.source() > ma.source() {
                        parent_ma
                    } else {
                        ma
                    }
                } else {
                    ma
                }
                .clone();
                vals_map.insert(global_arg.clone(), to_update);
            }
        }
        if let Some(ref mut sc) = self.matches.subcommand {
            let mut am = ArgMatcher {
                matches: mem::take(&mut sc.matches),
                pending: None,
            };
            am.fill_in_global_values(global_arg_vec, vals_map);
            mem::swap(&mut am.matches, &mut sc.matches);
        }

        for (name, matched_arg) in vals_map.iter_mut() {
            self.matches.args.insert(name.clone(), matched_arg.clone());
        }
    }

    pub(crate) fn get(&self, arg: &Id) -> Option<&MatchedArg> {
        self.matches.args.get(arg)
    }

    pub(crate) fn get_mut(&mut self, arg: &Id) -> Option<&mut MatchedArg> {
        self.matches.args.get_mut(arg)
    }

    pub(crate) fn remove(&mut self, arg: &Id) -> bool {
        self.matches.args.remove(arg).is_some()
    }

    pub(crate) fn contains(&self, arg: &Id) -> bool {
        self.matches.args.contains_key(arg)
    }

    pub(crate) fn arg_ids(&self) -> std::slice::Iter<'_, Id> {
        self.matches.args.keys()
    }

    pub(crate) fn args(&self) -> crate::util::flat_map::Iter<'_, Id, MatchedArg> {
        self.matches.args.iter()
    }

    pub(crate) fn entry(&mut self, arg: Id) -> crate::util::Entry<'_, Id, MatchedArg> {
        self.matches.args.entry(arg)
    }

    pub(crate) fn subcommand(&mut self, sc: SubCommand) {
        self.matches.subcommand = Some(Box::new(sc));
    }

    pub(crate) fn subcommand_name(&self) -> Option<&str> {
        self.matches.subcommand_name()
    }

    pub(crate) fn check_explicit(&self, arg: &Id, predicate: &ArgPredicate) -> bool {
        self.get(arg)
            .map(|a| a.check_explicit(predicate))
            .unwrap_or_default()
    }

    pub(crate) fn start_custom_arg(&mut self, arg: &Arg, source: ValueSource) {
        let id = arg.get_id().clone();
        debug!("ArgMatcher::start_custom_arg: id={id:?}, source={source:?}");
        let ma = self.entry(id).or_insert(MatchedArg::new_arg(arg));
        debug_assert_eq!(ma.type_id(), Some(arg.get_value_parser().type_id()));
        ma.set_source(source);
        ma.new_val_group();
    }

    pub(crate) fn start_custom_group(&mut self, id: Id, source: ValueSource) {
        debug!("ArgMatcher::start_custom_arg: id={id:?}, source={source:?}");
        let ma = self.entry(id).or_insert(MatchedArg::new_group());
        debug_assert_eq!(ma.type_id(), None);
        ma.set_source(source);
        ma.new_val_group();
    }

    pub(crate) fn start_occurrence_of_external(&mut self, cmd: &Command) {
        let id = Id::from_static_ref(Id::EXTERNAL);
        debug!("ArgMatcher::start_occurrence_of_external: id={id:?}");
        let ma = self.entry(id).or_insert(MatchedArg::new_external(cmd));
        debug_assert_eq!(
            ma.type_id(),
            Some(
                cmd.get_external_subcommand_value_parser()
                    .expect(INTERNAL_ERROR_MSG)
                    .type_id()
            )
        );
        ma.set_source(ValueSource::CommandLine);
        ma.new_val_group();
    }

    pub(crate) fn add_val_to(&mut self, arg: &Id, val: AnyValue, raw_val: OsString) {
        let ma = self.get_mut(arg).expect(INTERNAL_ERROR_MSG);
        ma.append_val(val, raw_val);
    }

    pub(crate) fn add_index_to(&mut self, arg: &Id, idx: usize) {
        let ma = self.get_mut(arg).expect(INTERNAL_ERROR_MSG);
        ma.push_index(idx);
    }

    pub(crate) fn needs_more_vals(&self, o: &Arg) -> bool {
        let num_pending = self
            .pending
            .as_ref()
            .and_then(|p| (p.id == *o.get_id()).then_some(p.raw_vals.len()))
            .unwrap_or(0);
        debug!(
            "ArgMatcher::needs_more_vals: o={}, pending={}",
            o.get_id(),
            num_pending
        );
        let expected = o.get_num_args().expect(INTERNAL_ERROR_MSG);
        debug!("ArgMatcher::needs_more_vals: expected={expected}, actual={num_pending}");
        expected.accepts_more(num_pending)
    }

    pub(crate) fn pending_arg_id(&self) -> Option<&Id> {
        self.pending.as_ref().map(|p| &p.id)
    }

    pub(crate) fn pending_values_mut(
        &mut self,
        id: &Id,
        ident: Option<Identifier>,
        trailing_values: bool,
    ) -> &mut Vec<OsString> {
        let pending = self.pending.get_or_insert_with(|| PendingArg {
            id: id.clone(),
            ident,
            raw_vals: Default::default(),
            trailing_idx: None,
        });
        debug_assert_eq!(pending.id, *id, "{INTERNAL_ERROR_MSG}");
        if ident.is_some() {
            debug_assert_eq!(pending.ident, ident, "{INTERNAL_ERROR_MSG}");
        }
        if trailing_values {
            pending.trailing_idx.get_or_insert(pending.raw_vals.len());
        }
        &mut pending.raw_vals
    }

    pub(crate) fn start_trailing(&mut self) {
        if let Some(pending) = &mut self.pending {
            // Allow asserting its started on subsequent calls
            pending.trailing_idx.get_or_insert(pending.raw_vals.len());
        }
    }

    pub(crate) fn take_pending(&mut self) -> Option<PendingArg> {
        self.pending.take()
    }
}

impl Deref for ArgMatcher {
    type Target = ArgMatches;

    fn deref(&self) -> &Self::Target {
        &self.matches
    }
}
