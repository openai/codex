mod util;
use std::marker::PhantomData;

use schemars::JsonSchema;
use util::*;

struct MyIterator;

impl Iterator for MyIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        unimplemented!()
    }
}

// The default trait bounds would require T to implement JsonSchema,
// which MyIterator does not.
#[derive(JsonSchema)]
#[schemars(bound = "T::Item: JsonSchema", rename = "MyContainer")]
pub struct MyContainer<T>
where
    T: Iterator,
{
    pub associated: T::Item,
    pub generic: PhantomData<T>,
}

#[test]
fn manual_bound_set() -> TestResult {
    test_default_generated_schema::<MyContainer<MyIterator>>("bound")
}
