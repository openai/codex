mod detect;
mod ledger;

use std::time::SystemTime;
use std::time::UNIX_EPOCH;

pub(crate) use detect::detect_recent_sessions;
pub(crate) use ledger::has_current_session_been_imported;
pub(crate) use ledger::record_imported_session;

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}
