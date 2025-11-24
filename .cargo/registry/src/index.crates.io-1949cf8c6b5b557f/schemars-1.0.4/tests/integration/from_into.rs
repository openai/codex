use std::num::ParseIntError;

use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(into = "String", try_from = "String")]
#[schemars(extend("pattern" = r"^[0-9a-f]{16}:[0-9a-f]{16}:[0-9a-f]{4}$"))]
pub struct EventSequenceNumber {
    transaction_id: u64,
    statement_id: u64,
    write_id: u16,
}

impl From<EventSequenceNumber> for String {
    fn from(value: EventSequenceNumber) -> Self {
        format!(
            "{:016x}:{:016x}:{:04x}",
            value.transaction_id, value.statement_id, value.write_id
        )
    }
}

impl TryFrom<String> for EventSequenceNumber {
    type Error = ParseIntError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let mut parts = value.split(':');
        Ok(Self {
            transaction_id: parts.next().unwrap_or_default().parse()?,
            statement_id: parts.next().unwrap_or_default().parse()?,
            write_id: parts.next().unwrap_or_default().parse()?,
        })
    }
}

#[test]
fn into_and_try_from() {
    test!(EventSequenceNumber)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(from = "i128")]
struct ClampingI8(i8);

impl From<i128> for ClampingI8 {
    fn from(value: i128) -> Self {
        Self(if value > i8::MAX as i128 {
            i8::MAX
        } else if value < i8::MIN as i128 {
            i8::MIN
        } else {
            value as i8
        })
    }
}

#[test]
fn from() {
    test!(ClampingI8)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}
