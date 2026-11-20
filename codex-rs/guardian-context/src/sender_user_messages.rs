//! Both reviewers consume the same host-rendered, delivery-bound sender evidence.

use codex_history::RetainedContext;

use crate::ContextSection;
use crate::SectionContributor;
use crate::SectionError;
use crate::SectionInput;
use crate::SectionScope;

pub(crate) struct SenderUserMessagesSection;

impl SectionContributor for SenderUserMessagesSection {
    fn scope(&self) -> SectionScope {
        SectionScope::Shared
    }

    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError> {
        Ok(input
            .history
            .retained_context()
            .and_then(RetainedContext::sender_user_messages)
            .map(|snapshot| {
                let text = if snapshot.text.is_empty() {
                    "Host: No sender evidence fits the configured budget. Do not infer permission from missing sender evidence.\n".to_owned()
                } else {
                    snapshot.text.clone()
                };
                ContextSection::SenderUserMessages { items: vec![text] }
            }))
    }
}
