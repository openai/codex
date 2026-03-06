pub(crate) mod log;
pub(crate) mod trace;

use chrono::SecondsFormat;
use chrono::Utc;

pub(crate) fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
