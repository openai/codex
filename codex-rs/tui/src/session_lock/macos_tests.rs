#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn screen_saver_stops_voice_when_its_password_grace_period_expires() {
    let password_required_at = 1_793_735_319_i64;
    let session = CFDictionary::from_CFType_pairs(&[
        (
            CFString::new("CGSSessionScreenIsLocked"),
            CFBoolean::true_value().as_CFType(),
        ),
        (
            CFString::new("CGSSessionScreenLockedTime"),
            CFNumber::from(password_required_at).as_CFType(),
        ),
    ]);
    let observed = [password_required_at - 1, password_required_at]
        .map(|now| requires_authentication(&session, now));
    assert_eq!(observed, [false, true]);
}
