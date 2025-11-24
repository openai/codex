use crate::prelude::*;
use schemars::generate::SchemaSettings;

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct MyJob {
    spec: MyJobSpec,
}

#[derive(JsonSchema, Deserialize, Serialize, Default)]
struct MyJobSpec {
    replicas: u32,
}

#[test]
fn struct_normal() {
    let settings = SchemaSettings::default().with(|s| s.inline_subschemas = true);
    test!(MyJob, settings)
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_matches_de_roundtrip(arbitrary_values());
}

#[derive(JsonSchema, Deserialize, Serialize)]
struct RecursiveOuter {
    direct: Option<Box<RecursiveOuter>>,
    indirect: Option<Box<RecursiveInner>>,
}

#[derive(JsonSchema, Deserialize, Serialize)]
struct RecursiveInner {
    recursive: RecursiveOuter,
}

#[test]
fn struct_recursive() {
    let settings = SchemaSettings::default().with(|s| s.inline_subschemas = true);
    test!(RecursiveOuter, settings)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([
            RecursiveOuter {
                direct: None,
                indirect: None,
            },
            RecursiveOuter {
                direct: Some(Box::new(RecursiveOuter {
                    direct: None,
                    indirect: None,
                })),
                indirect: Some(Box::new(RecursiveInner {
                    recursive: RecursiveOuter {
                        direct: Some(Box::new(RecursiveOuter {
                            direct: None,
                            indirect: None,
                        })),
                        indirect: None,
                    },
                })),
            },
        ])
        .assert_matches_de_roundtrip(arbitrary_values());
}
