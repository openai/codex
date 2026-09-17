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
            .map(|snapshot| ContextSection::SenderUserMessages {
                items: vec![snapshot.text.clone()],
            }))
    }
}
