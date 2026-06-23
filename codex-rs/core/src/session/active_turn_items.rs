use std::collections::HashMap;

use codex_protocol::items::TurnItem;

pub(super) struct ActiveTurnItem {
    pub(super) item: TurnItem,
    pub(super) streams_to_client: bool,
}

#[derive(Default)]
pub(super) struct ActiveTurnItems {
    by_response_item_id: HashMap<String, ActiveTurnItem>,
    unkeyed: Option<ActiveTurnItem>,
    current_response_item_id: Option<String>,
}

impl ActiveTurnItems {
    pub(super) fn mark_started(&mut self, response_item_id: Option<&str>) {
        self.current_response_item_id = response_item_id.map(str::to_owned);
    }

    pub(super) fn insert(
        &mut self,
        response_item_id: Option<&str>,
        item: TurnItem,
        streams_to_client: bool,
    ) {
        let active = ActiveTurnItem {
            item,
            streams_to_client,
        };
        if let Some(response_item_id) = response_item_id {
            self.unkeyed = None;
            self.by_response_item_id
                .insert(response_item_id.to_owned(), active);
        } else {
            self.unkeyed = Some(active);
        }
    }

    pub(super) fn get(&self, response_item_id: Option<&str>) -> Option<&ActiveTurnItem> {
        match response_item_id {
            Some(response_item_id) => self.by_response_item_id.get(response_item_id),
            None => self
                .current_response_item_id
                .as_deref()
                .and_then(|item_id| self.by_response_item_id.get(item_id))
                .or(self.unkeyed.as_ref()),
        }
    }

    pub(super) fn take(&mut self, response_item_id: Option<&str>) -> Option<ActiveTurnItem> {
        match response_item_id {
            Some(response_item_id) => {
                if self.current_response_item_id.as_deref() == Some(response_item_id) {
                    self.current_response_item_id = None;
                }
                self.by_response_item_id.remove(response_item_id)
            }
            None => {
                if let Some(response_item_id) = self.current_response_item_id.take() {
                    self.by_response_item_id.remove(&response_item_id)
                } else {
                    self.unkeyed.take()
                }
            }
        }
    }

    pub(super) fn is_current(&self, response_item_id: &str) -> bool {
        self.current_response_item_id.as_deref() == Some(response_item_id)
    }
}
