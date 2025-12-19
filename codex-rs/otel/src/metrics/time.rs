use std::time::Duration;

pub(crate) fn duration_to_millis(duration: Duration) -> i64 {
    let millis = duration.as_millis();
    let capped = millis.min(i64::MAX as u128);
    capped as i64
}
