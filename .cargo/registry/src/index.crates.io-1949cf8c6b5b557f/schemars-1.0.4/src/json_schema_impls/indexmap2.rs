use crate::JsonSchema;
use alloc::collections::{BTreeMap, BTreeSet};
use indexmap2::{IndexMap, IndexSet};

forward_impl!((<K: JsonSchema, V: JsonSchema, H> JsonSchema for IndexMap<K, V, H>) => BTreeMap<K, V>);
forward_impl!((<T: JsonSchema, H> JsonSchema for IndexSet<T, H>) => BTreeSet<T>);
