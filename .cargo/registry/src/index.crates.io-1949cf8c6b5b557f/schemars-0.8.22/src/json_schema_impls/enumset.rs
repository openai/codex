use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use enumset::{EnumSet, EnumSetType};

forward_impl!((<T> JsonSchema for EnumSet<T> where T: EnumSetType + JsonSchema) => std::collections::BTreeSet<T>);
