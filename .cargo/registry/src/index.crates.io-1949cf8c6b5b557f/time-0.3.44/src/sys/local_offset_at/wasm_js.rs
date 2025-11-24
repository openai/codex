use num_conv::prelude::*;

use crate::convert::*;
use crate::{OffsetDateTime, UtcOffset};

/// Obtain the system's UTC offset.
#[inline]
pub(super) fn local_offset_at(datetime: OffsetDateTime) -> Option<UtcOffset> {
    let js_date: js_sys::Date = datetime.into();
    // The number of minutes returned by getTimezoneOffset() is positive if the local time zone
    // is behind UTC, and negative if the local time zone is ahead of UTC. For example,
    // for UTC+10, -600 will be returned.
    let timezone_offset = (js_date.get_timezone_offset() as i32) * -Minute::per_t::<i32>(Hour);

    UtcOffset::from_whole_seconds(timezone_offset).ok()
}
