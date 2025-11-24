#![allow(dead_code)]

//! Test expansion of enums which have struct variants.

use darling::FromMeta;
#[derive(Debug, FromMeta)]
#[darling(rename_all = "snake_case")]
enum Message {
    Hello { user: String, silent: bool },
    Ping,
    Goodbye { user: String },
}

#[test]
fn expansion() {}
