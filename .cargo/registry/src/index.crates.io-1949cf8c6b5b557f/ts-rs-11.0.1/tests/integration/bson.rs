#![cfg(feature = "bson-uuid-impl")]

use bson::{oid::ObjectId, Uuid};
use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "bson/")]
struct User {
    _id: ObjectId,
    _uuid: Uuid,
}

#[test]
fn bson() {
    assert_eq!(User::decl(), "type User = { _id: string, _uuid: string, };")
}
