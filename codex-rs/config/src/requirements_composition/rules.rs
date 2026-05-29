//! Requirements rules are additive across layers. Higher-priority rules are
//! appended first so the final rule order keeps priority visible.

use crate::RequirementSource;
use crate::RequirementsExecPolicyToml;
use crate::Sourced;

use super::stack::merge_output_source;

pub(super) fn merge(
    target: &mut Option<Sourced<RequirementsExecPolicyToml>>,
    incoming: Option<RequirementsExecPolicyToml>,
    source: &RequirementSource,
) {
    let Some(incoming) = incoming else {
        return;
    };
    let Some(existing) = target.as_mut() else {
        *target = Some(Sourced::new(incoming, source.clone()));
        return;
    };

    let RequirementsExecPolicyToml { prefix_rules } = incoming;
    existing.value.prefix_rules.extend(prefix_rules);
    merge_output_source(&mut existing.source, source);
}
