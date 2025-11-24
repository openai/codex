use crate::prelude::*;
use std::marker::PhantomData;

struct MyIterator;

impl Iterator for MyIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        unimplemented!()
    }
}

// The default trait bounds would require T to implement JsonSchema,
// which MyIterator does not.
#[derive(JsonSchema, Serialize, Deserialize)]
#[schemars(bound = "T::Item: JsonSchema", rename = "MyContainer")]
pub struct MyContainer<T>
where
    T: Iterator,
{
    pub associated: T::Item,
    pub generic: PhantomData<T>,
}

#[test]
fn manual_bound_set() {
    test!(MyContainer<MyIterator>)
        .assert_snapshot()
        .assert_allows_ser_roundtrip([MyContainer {
            associated: "test".to_owned(),
            generic: PhantomData,
        }])
        .assert_matches_de_roundtrip(arbitrary_values());
}
