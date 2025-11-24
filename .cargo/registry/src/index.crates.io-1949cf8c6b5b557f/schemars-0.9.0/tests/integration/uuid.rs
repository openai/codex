use crate::prelude::*;
use schemars::generate::SchemaSettings;
use uuid1::Uuid;

#[test]
fn uuid() {
    // Must run with draft 2019-09 due to https://github.com/Stranger6667/jsonschema-rs/issues/456
    test!(Uuid, SchemaSettings::draft2019_09())
        .assert_snapshot()
        .assert_allows_ser_roundtrip([Uuid::nil(), Uuid::max(), Uuid::from_u128(1234567890)])
        .assert_matches_de_roundtrip(arbitrary_values());
}
