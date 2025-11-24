#![cfg_attr(docsrs, doc(cfg(feature = "sval")))]

use crate::{IndexMap, IndexSet};
use sval::{Stream, Value};

impl<K: Value, V: Value, S> Value for IndexMap<K, V, S> {
    fn stream<'sval, ST: Stream<'sval> + ?Sized>(&'sval self, stream: &mut ST) -> sval::Result {
        stream.map_begin(Some(self.len()))?;

        for (k, v) in self {
            stream.map_key_begin()?;
            stream.value(k)?;
            stream.map_key_end()?;

            stream.map_value_begin()?;
            stream.value(v)?;
            stream.map_value_end()?;
        }

        stream.map_end()
    }
}

impl<K: Value, S> Value for IndexSet<K, S> {
    fn stream<'sval, ST: Stream<'sval> + ?Sized>(&'sval self, stream: &mut ST) -> sval::Result {
        stream.seq_begin(Some(self.len()))?;

        for value in self {
            stream.seq_value_begin()?;
            stream.value(value)?;
            stream.seq_value_end()?;
        }

        stream.seq_end()
    }
}
