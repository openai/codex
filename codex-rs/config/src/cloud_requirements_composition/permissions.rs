use super::CloudRequirementsFragmentSource;
use super::merge_output_source;
use crate::Sourced;
use crate::config_requirements::FilesystemRequirementsToml;
use crate::config_requirements::PermissionsRequirementsToml;

// Permissions compose filesystem deny_read requirements and permission profile
// definitions. The deny_read list is a stable union in bundle order, with
// duplicates removed. Profile definitions merge by key, with the first
// definition winning because cloud fragments are already priority ordered.

pub(super) fn merge_permissions(
    target: &mut Option<Sourced<PermissionsRequirementsToml>>,
    incoming: Option<PermissionsRequirementsToml>,
    source_ref: &CloudRequirementsFragmentSource,
) {
    let Some(incoming) = incoming.filter(permissions_has_mergeable_content) else {
        return;
    };
    let Some(existing) = target.as_mut() else {
        *target = Some(Sourced::new(incoming, source_ref.requirement_source()));
        return;
    };

    if merge_permissions_requirements(&mut existing.value, incoming) {
        merge_output_source(&mut existing.source, source_ref);
    }
}

fn permissions_has_mergeable_content(permissions: &PermissionsRequirementsToml) -> bool {
    if !permissions.profiles.is_empty() {
        return true;
    }

    permissions
        .filesystem
        .as_ref()
        .and_then(|filesystem| filesystem.deny_read.as_ref())
        .is_some_and(|deny_read| !deny_read.is_empty())
}

fn merge_permissions_requirements(
    existing: &mut PermissionsRequirementsToml,
    incoming: PermissionsRequirementsToml,
) -> bool {
    // Destructure without `..` so new permission families cannot bypass cloud
    // composition without an explicit merge policy.
    let PermissionsRequirementsToml {
        filesystem,
        profiles,
    } = incoming;
    let mut changed = false;
    for (profile_name, profile) in profiles {
        if let std::collections::btree_map::Entry::Vacant(entry) =
            existing.profiles.entry(profile_name)
        {
            entry.insert(profile);
            changed = true;
        }
    }

    // Destructure without `..` so new filesystem permission fields must choose
    // their own merge behavior.
    let Some(FilesystemRequirementsToml { deny_read }) = filesystem else {
        return changed;
    };
    let Some(incoming_deny_read) = deny_read.filter(|patterns| !patterns.is_empty()) else {
        return changed;
    };

    let existing_filesystem = existing.filesystem.get_or_insert_with(Default::default);
    let existing_deny_read = existing_filesystem.deny_read.get_or_insert_with(Vec::new);
    for pattern in incoming_deny_read {
        if !existing_deny_read.contains(&pattern) {
            existing_deny_read.push(pattern);
            changed = true;
        }
    }
    changed
}
