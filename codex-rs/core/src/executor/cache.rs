use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, Default)]
/// Thread-safe store of user approvals so repeated commands can reuse
/// previously granted trust.
pub(crate) struct ApprovalCache {
    inner: Arc<Mutex<HashSet<Vec<String>>>>,
}

impl ApprovalCache {
    pub(crate) fn insert(&self, command: Vec<String>) {
        if command.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(command);
        }
    }

    pub(crate) fn snapshot(&self) -> HashSet<Vec<String>> {
        self.inner.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn insert_ignores_empty_and_dedupes() {
        let cache = ApprovalCache::default();

        // Empty should be ignored
        cache.insert(vec![]);
        assert!(cache.snapshot().is_empty());

        // Insert a command and verify snapshot contains it
        let cmd = vec!["foo".to_string(), "bar".to_string()];
        cache.insert(cmd.clone());
        let snap1 = cache.snapshot();
        assert!(snap1.contains(&cmd));

        // Reinserting should not create duplicates
        cache.insert(cmd);
        let snap2 = cache.snapshot();
        assert_eq!(snap1, snap2);
    }
}
