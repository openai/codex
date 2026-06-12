use std::collections::HashMap;

use codex_protocol::protocol::GuardianAssessmentDecisionSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardianRejection {
    pub rationale: String,
    pub source: GuardianAssessmentDecisionSource,
}

/// Rejection rationales awaiting delivery to callers for one turn.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct GuardianRejectionStore {
    rejections: HashMap<String, GuardianRejection>,
}

impl GuardianRejectionStore {
    pub fn insert(
        &mut self,
        review_id: String,
        rejection: GuardianRejection,
    ) -> Option<GuardianRejection> {
        self.rejections.insert(review_id, rejection)
    }

    pub fn remove(&mut self, review_id: &str) -> Option<GuardianRejection> {
        self.rejections.remove(review_id)
    }

    pub fn contains(&self, review_id: &str) -> bool {
        self.rejections.contains_key(review_id)
    }
}

#[cfg(test)]
#[path = "rejection_store_tests.rs"]
mod tests;
