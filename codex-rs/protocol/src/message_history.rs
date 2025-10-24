use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
pub struct HistoryEntry {
    pub conversation_id: String,
    pub ts: u64,
    pub text: String,
}
