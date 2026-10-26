//! Prepares synchronous request evidence before final aggregate admission.
//! Collection keeps captured sources intact. This owner applies text and image policy;
//! delivery checks repeat against current reviewer history after compaction.

use crate::CollectedContext;
use crate::ContextSection;

/// Applies synchronous section policy to the already bounded collection snapshot.
/// The host prepares its borrowed transcript selection before consuming the collection.
pub(crate) fn prepare_sync_sections(context: &mut CollectedContext) {
    for section in &mut context.sections {
        if let ContextSection::NodeReplEvidence(evidence) = section
            && let crate::node_repl::NodeReplEvidenceContent::Collected {
                responses,
                omitted_responses,
                mode,
            } = &evidence.content
        {
            let snapshot = crate::NodeReplContext {
                responses,
                omitted_responses: *omitted_responses,
                mode: *mode,
            };
            let items = snapshot.render_inputs();
            evidence.content = crate::node_repl::NodeReplEvidenceContent::Prepared(items);
        }
    }
}
