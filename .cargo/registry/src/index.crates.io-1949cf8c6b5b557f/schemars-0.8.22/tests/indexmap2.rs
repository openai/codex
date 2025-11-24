mod util;
use indexmap2::{IndexMap, IndexSet};
use schemars::JsonSchema;
use util::*;

#[allow(dead_code)]
#[derive(JsonSchema)]
struct IndexMapTypes {
    map: IndexMap<i32, bool>,
    set: IndexSet<isize>,
}

#[test]
fn indexmap_types() -> TestResult {
    test_default_generated_schema::<IndexMapTypes>("indexmap")
}
