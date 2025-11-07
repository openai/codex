use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;

use codex_protocol::ConversationId;

use crate::codex::Session;

struct IndexInner {
    map: HashMap<ConversationId, Weak<Session>>,
}

impl IndexInner {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

static INDEX: OnceLock<Mutex<IndexInner>> = OnceLock::new();

fn idx() -> &'static Mutex<IndexInner> {
    INDEX.get_or_init(|| Mutex::new(IndexInner::new()))
}

pub(crate) fn register(conversation_id: ConversationId, session: &Arc<Session>) {
    if let Ok(mut guard) = idx().lock() {
        guard.map.insert(conversation_id, Arc::downgrade(session));
    }
}

pub(crate) fn get(conversation_id: &ConversationId) -> Option<Arc<Session>> {
    let mut guard = idx().lock().ok()?;
    match guard.map.get(conversation_id) {
        Some(w) => w.upgrade().or_else(|| {
            guard.map.remove(conversation_id);
            None
        }),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prunes_stale_sessions() {
        let conversation_id = ConversationId::new();
        {
            let mut guard = idx().lock().unwrap();
            guard.map.insert(conversation_id, Weak::new());
        }

        // First lookup should detect the dead weak ptr, prune it, and return None.
        assert!(get(&conversation_id).is_none());

        // Second lookup should see the map entry removed.
        {
            let guard = idx().lock().unwrap();
            assert!(!guard.map.contains_key(&conversation_id));
        }
    }
}
