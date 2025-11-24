use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use indexmap2::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};

forward_impl!((<K, V: JsonSchema, H> JsonSchema for IndexMap<K, V, H>) => HashMap<K, V, H>);
forward_impl!((<T: JsonSchema, H> JsonSchema for IndexSet<T, H>) => HashSet<T, H>);
