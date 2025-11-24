use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use smallvec::{Array, SmallVec};

forward_impl!((<A: Array> JsonSchema for SmallVec<A> where A::Item: JsonSchema) => Vec<A::Item>);
