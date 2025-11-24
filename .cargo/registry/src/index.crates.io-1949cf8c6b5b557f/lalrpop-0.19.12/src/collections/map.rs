use std::collections::BTreeMap;

pub use std::collections::btree_map::Entry;

/// In general, we avoid coding directly against any particular map,
/// but rather build against `util::Map` (and `util::map` to construct
/// an instance). This should be a deterministic map, such that two
/// runs of LALRPOP produce the same output, but otherwise it doesn't
/// matter much. I'd probably prefer to use `HashMap` with an
/// alternative hasher, but that's not stable.
pub type Map<K, V> = BTreeMap<K, V>;

pub fn map<K: Ord, V>() -> Map<K, V> {
    Map::<K, V>::default()
}
