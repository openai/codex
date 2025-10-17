use codex_protocol::items::TurnItem;
use std::collections::HashMap;

pub(crate) struct ItemCollector {
    items: HashMap<String, TurnItem>,
}

impl ItemCollector {
    pub fn new() -> ItemCollector {
        ItemCollector {
            items: HashMap::new(),
        }
    }

    pub fn started(&mut self, item: TurnItem) {
        self.items.insert(item.id(), item);
    }

    pub fn completed(&mut self, item: TurnItem) {
        self.items.remove(&item.id());
    }
}
