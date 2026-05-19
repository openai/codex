use super::CloudRequirementsCompositionError;
use super::CloudRequirementsFragmentSource;
use super::composition_conflict;
use super::merge_output_source;
use crate::HookEventsToml;
use crate::ManagedHooksRequirementsToml;
use crate::Sourced;
use std::collections::BTreeMap;
use std::path::PathBuf;

// Hook events are append-only across cloud layers. The managed hook directory is
// different: only one directory is usable on a given platform, so conflicting
// values for the active platform fail closed. The inactive platform field is
// first-filled to allow the same bundle to carry OS-specific directories.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum HookDirectoryField {
    #[default]
    ManagedDir,
    WindowsManagedDir,
}

impl HookDirectoryField {
    pub(super) fn current_platform() -> Self {
        if cfg!(windows) {
            Self::WindowsManagedDir
        } else {
            Self::ManagedDir
        }
    }

    fn field_name(self) -> &'static str {
        match self {
            Self::ManagedDir => "hooks.managed_dir",
            Self::WindowsManagedDir => "hooks.windows_managed_dir",
        }
    }

    fn inactive(self) -> Self {
        match self {
            Self::ManagedDir => Self::WindowsManagedDir,
            Self::WindowsManagedDir => Self::ManagedDir,
        }
    }
}

pub(super) struct HookMergeState {
    directory_field: HookDirectoryField,
    dir_sources: BTreeMap<HookDirectoryField, CloudRequirementsFragmentSource>,
}

impl HookMergeState {
    pub(super) fn new(directory_field: HookDirectoryField) -> Self {
        Self {
            directory_field,
            dir_sources: BTreeMap::new(),
        }
    }

    pub(super) fn merge(
        &mut self,
        target: &mut Option<Sourced<ManagedHooksRequirementsToml>>,
        incoming: Option<ManagedHooksRequirementsToml>,
        source_ref: &CloudRequirementsFragmentSource,
    ) -> Result<(), CloudRequirementsCompositionError> {
        let Some(mut incoming) = incoming.filter(|value| !value.is_empty()) else {
            return Ok(());
        };
        let Some(existing) = target.as_mut() else {
            self.track_singleton_source(
                HookDirectoryField::ManagedDir,
                &incoming.managed_dir,
                source_ref,
            );
            self.track_singleton_source(
                HookDirectoryField::WindowsManagedDir,
                &incoming.windows_managed_dir,
                source_ref,
            );
            *target = Some(Sourced::new(incoming, source_ref.requirement_source()));
            return Ok(());
        };

        let active_field = self.directory_field;
        let inactive_field = active_field.inactive();
        let incoming_active_dir = take_hook_dir(&mut incoming, active_field);
        let incoming_inactive_dir = take_hook_dir(&mut incoming, inactive_field);
        let mut changed = false;
        changed |= self.merge_active_singleton(
            active_field,
            hook_dir_mut(&mut existing.value, active_field),
            incoming_active_dir,
            source_ref,
        )?;
        changed |= self.fill_singleton(
            inactive_field,
            hook_dir_mut(&mut existing.value, inactive_field),
            incoming_inactive_dir,
            source_ref,
        );
        changed |= append_hook_events(&mut existing.value.hooks, incoming.hooks);
        if changed {
            merge_output_source(&mut existing.source, source_ref);
        }
        Ok(())
    }

    fn track_singleton_source(
        &mut self,
        field: HookDirectoryField,
        value: &Option<PathBuf>,
        source_ref: &CloudRequirementsFragmentSource,
    ) {
        if value.is_some() {
            self.dir_sources
                .entry(field)
                .or_insert_with(|| source_ref.clone());
        }
    }

    fn merge_active_singleton(
        &mut self,
        field: HookDirectoryField,
        existing: &mut Option<PathBuf>,
        incoming: Option<PathBuf>,
        incoming_source: &CloudRequirementsFragmentSource,
    ) -> Result<bool, CloudRequirementsCompositionError> {
        let Some(incoming) = incoming else {
            return Ok(false);
        };

        match existing {
            Some(existing_value) if existing_value != &incoming => {
                let existing_source = self
                    .dir_sources
                    .get(&field)
                    .cloned()
                    .unwrap_or_else(|| incoming_source.clone());
                Err(composition_conflict(
                    field.field_name().to_string(),
                    existing_source,
                    incoming_source.clone(),
                    format!(
                        "`{}` conflicts with `{}`",
                        existing_value.display(),
                        incoming.display()
                    ),
                ))
            }
            Some(_) => Ok(false),
            None => {
                *existing = Some(incoming);
                self.dir_sources
                    .entry(field)
                    .or_insert_with(|| incoming_source.clone());
                Ok(true)
            }
        }
    }

    fn fill_singleton(
        &mut self,
        field: HookDirectoryField,
        existing: &mut Option<PathBuf>,
        incoming: Option<PathBuf>,
        incoming_source: &CloudRequirementsFragmentSource,
    ) -> bool {
        if existing.is_none()
            && let Some(incoming) = incoming
        {
            *existing = Some(incoming);
            self.dir_sources
                .entry(field)
                .or_insert_with(|| incoming_source.clone());
            return true;
        }
        false
    }
}

fn take_hook_dir(
    hooks: &mut ManagedHooksRequirementsToml,
    field: HookDirectoryField,
) -> Option<PathBuf> {
    match field {
        HookDirectoryField::ManagedDir => hooks.managed_dir.take(),
        HookDirectoryField::WindowsManagedDir => hooks.windows_managed_dir.take(),
    }
}

fn hook_dir_mut(
    hooks: &mut ManagedHooksRequirementsToml,
    field: HookDirectoryField,
) -> &mut Option<PathBuf> {
    match field {
        HookDirectoryField::ManagedDir => &mut hooks.managed_dir,
        HookDirectoryField::WindowsManagedDir => &mut hooks.windows_managed_dir,
    }
}

fn append_hook_events(existing: &mut HookEventsToml, incoming: HookEventsToml) -> bool {
    // Destructure without `..` so new hook events cannot be introduced without
    // deciding whether cloud composition should append them.
    let HookEventsToml {
        pre_tool_use,
        permission_request,
        post_tool_use,
        pre_compact,
        post_compact,
        session_start,
        user_prompt_submit,
        subagent_start,
        subagent_stop,
        stop,
    } = incoming;

    let mut changed = false;
    changed |= append_vec(&mut existing.pre_tool_use, pre_tool_use);
    changed |= append_vec(&mut existing.permission_request, permission_request);
    changed |= append_vec(&mut existing.post_tool_use, post_tool_use);
    changed |= append_vec(&mut existing.pre_compact, pre_compact);
    changed |= append_vec(&mut existing.post_compact, post_compact);
    changed |= append_vec(&mut existing.session_start, session_start);
    changed |= append_vec(&mut existing.user_prompt_submit, user_prompt_submit);
    changed |= append_vec(&mut existing.subagent_start, subagent_start);
    changed |= append_vec(&mut existing.subagent_stop, subagent_stop);
    changed |= append_vec(&mut existing.stop, stop);
    changed
}

fn append_vec<T>(existing: &mut Vec<T>, mut incoming: Vec<T>) -> bool {
    let changed = !incoming.is_empty();
    existing.append(&mut incoming);
    changed
}
