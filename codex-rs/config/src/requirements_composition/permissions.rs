//! `permissions.filesystem.deny_read` is intentionally additive across
//! requirements layers. Other `[permissions]` content stays in the regular TOML
//! merge path so permission profile tables follow config-style precedence.

use crate::FilesystemDenyReadPattern;
use crate::RequirementSource;
use crate::Sourced;
use crate::config_requirements::FilesystemRequirementsToml;
use crate::config_requirements::PermissionsRequirementsToml;

use super::stack::merge_output_source;

#[derive(Default)]
pub(super) struct DenyReadMergeState {
    deny_read: Vec<FilesystemDenyReadPattern>,
    source: Option<RequirementSource>,
}

impl DenyReadMergeState {
    pub(super) fn merge(
        &mut self,
        incoming: Option<PermissionsRequirementsToml>,
        source: &RequirementSource,
    ) {
        let Some(incoming_deny_read) = incoming
            .and_then(|permissions| permissions.filesystem)
            .and_then(|filesystem| filesystem.deny_read)
            .filter(|deny_read| !deny_read.is_empty())
        else {
            return;
        };

        for pattern in incoming_deny_read {
            if !self.deny_read.contains(&pattern) {
                self.deny_read.push(pattern);
                self.merge_source(source);
            }
        }
    }

    pub(super) fn apply_to(self, target: &mut Option<Sourced<PermissionsRequirementsToml>>) {
        if self.deny_read.is_empty() {
            return;
        }

        let source = self.source.unwrap_or(RequirementSource::Unknown);
        let Some(existing) = target.as_mut() else {
            *target = Some(Sourced::new(
                PermissionsRequirementsToml {
                    filesystem: Some(FilesystemRequirementsToml {
                        deny_read: Some(self.deny_read),
                    }),
                    profiles: Default::default(),
                },
                source,
            ));
            return;
        };

        let filesystem = existing
            .value
            .filesystem
            .get_or_insert_with(Default::default);
        let deny_read = filesystem.deny_read.get_or_insert_with(Vec::new);
        for pattern in self.deny_read {
            if !deny_read.contains(&pattern) {
                deny_read.push(pattern);
            }
        }
        if existing.source != source {
            existing.source = RequirementSource::composite([existing.source.clone(), source]);
        }
    }

    fn merge_source(&mut self, source: &RequirementSource) {
        let Some(existing) = self.source.as_mut() else {
            self.source = Some(source.clone());
            return;
        };
        merge_output_source(existing, source);
    }
}
