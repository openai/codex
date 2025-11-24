use crate::prelude::*;
use std::marker::PhantomData;

#[derive(Default)]
struct MyIterator;

impl Iterator for MyIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        unimplemented!()
    }
}

// The default trait bounds would require T/U to implement JsonSchema, which MyIterator does not.
#[derive(JsonSchema, Serialize, Deserialize, Default)]
#[schemars(bound = "T::Item: JsonSchema")]
pub struct MyContainer<T, U>
where
    T: Iterator,
    U: Iterator,
{
    pub associated: T::Item,
    #[schemars(bound = "U::Item: JsonSchema")]
    pub associated2: U::Item,
    pub phantom: PhantomData<T>,
    #[serde(skip)]
    pub _skipped: T,
}

#[test]
fn manual_bound_set() {
    test!(MyContainer<MyIterator, MyIterator>)
        // TODO with better bounds, this assertion would work:
        // .assert_identical::<MyContainer<core::slice::Iter<&str>>>()
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_allows_ser_roundtrip([MyContainer {
            associated: "test".to_owned(),
            associated2: "test".to_owned(),
            phantom: PhantomData,
            _skipped: MyIterator,
        }])
        .assert_matches_de_roundtrip(arbitrary_values());

    assert_ne!(
        <MyContainer<MyIterator, MyIterator>>::schema_id(),
        <MyContainer<core::slice::Iter<i32>, MyIterator>>::schema_id()
    );
    assert_ne!(
        <MyContainer<MyIterator, MyIterator>>::schema_id(),
        <MyContainer<MyIterator, core::slice::Iter<i32>>>::schema_id()
    );
}

// `T` doesn't need to impl `JsonSchema`, but `U` does
#[derive(JsonSchema, Serialize, Deserialize, Default)]
pub struct MyContainer2<T, U>
where
    T: Iterator,
{
    pub u: Option<U>,
    pub phantom: PhantomData<T>,
    #[serde(skip)]
    pub _skipped: T,
}

#[test]
fn auto_bound() {
    test!(MyContainer2<MyIterator, String>)
        .assert_identical::<MyContainer2<core::slice::Iter<&str>, Box<str>>>()
        .assert_snapshot()
        .assert_allows_ser_roundtrip_default()
        .assert_allows_ser_roundtrip([MyContainer2 {
            u: Some("test".to_owned()),
            phantom: PhantomData,
            _skipped: MyIterator,
        }])
        .assert_matches_de_roundtrip(arbitrary_values());
}
