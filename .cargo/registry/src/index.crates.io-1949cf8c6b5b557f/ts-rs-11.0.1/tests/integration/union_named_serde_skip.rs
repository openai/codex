#![allow(dead_code)]

#[cfg(feature = "serde-compat")]
use serde::Deserialize;
use ts_rs::TS;

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Deserialize))]
#[cfg_attr(feature = "serde-compat", serde(untagged))]
#[cfg_attr(not(feature = "serde-compat"), ts(untagged))]
#[ts(export, export_to = "union_named_serde/")]
enum TestUntagged {
    A,   // serde_json -> `null`
    B(), // serde_json -> `[]`
    C {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        val: i32,
    }, // serde_json -> `{}`
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Deserialize))]
#[ts(export, export_to = "union_named_serde/")]
enum TestExternally {
    A,   // serde_json -> `"A"`
    B(), // serde_json -> `{"B":[]}`
    C {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        val: i32,
    }, // serde_json -> `{"C":{}}`
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Deserialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "type", content = "content"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "type", content = "content"))]
#[ts(export, export_to = "union_named_serde/")]
enum TestAdjacently {
    A,   // serde_json -> `{"type":"A"}`
    B(), // serde_json -> `{"type":"B","content":[]}`
    C {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        val: i32,
    }, // serde_json -> `{"type":"C","content":{}}`
}

#[derive(TS)]
#[cfg_attr(feature = "serde-compat", derive(Deserialize))]
#[cfg_attr(feature = "serde-compat", serde(tag = "type"))]
#[cfg_attr(not(feature = "serde-compat"), ts(tag = "type"))]
#[ts(export, export_to = "union_named_serde/")]
enum TestInternally {
    A, // serde_json -> `{"type":"A"}`
    B, // serde_json -> `{"type":"B"}`
    C {
        #[cfg_attr(feature = "serde-compat", serde(skip))]
        #[cfg_attr(not(feature = "serde-compat"), ts(skip))]
        val: i32,
    }, // serde_json -> `{"type":"C"}`
}

#[test]
fn test() {
    assert_eq!(
        TestUntagged::decl(),
        r#"type TestUntagged = null | never[] | {  };"#
    );

    assert_eq!(
        TestExternally::decl(),
        r#"type TestExternally = "A" | { "B": never[] } | { "C": {  } };"#
    );

    assert_eq!(
        TestAdjacently::decl(),
        r#"type TestAdjacently = { "type": "A" } | { "type": "B", "content": never[] } | { "type": "C", "content": {  } };"#
    );

    assert_eq!(
        TestInternally::decl(),
        r#"type TestInternally = { "type": "A" } | { "type": "B" } | { "type": "C", };"#
    );
}
