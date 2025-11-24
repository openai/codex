/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "hashbrown")]

use std::mem;

use hashbrown::raw::RawTable;
use hashbrown::HashTable;

use crate::Allocative;
use crate::Key;
use crate::Visitor;

const CAPACITY_NAME: Key = Key::new("capacity");

impl<T: Allocative> Allocative for RawTable<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        use crate::impls::common::DATA_NAME;
        use crate::impls::hashbrown_util;

        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter_unique(DATA_NAME, mem::size_of::<*const T>());
            {
                let mut visitor = visitor.enter(
                    CAPACITY_NAME,
                    hashbrown_util::raw_table_alloc_size_for_len::<T>(self.capacity()),
                );
                unsafe { visitor.visit_iter(self.iter().map(|e| e.as_ref())) };
                visitor.exit();
            }
            visitor.exit();
        }
        visitor.exit();
    }
}

impl<T: Allocative> Allocative for HashTable<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        use crate::impls::common::DATA_NAME;
        use crate::impls::hashbrown_util;

        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let mut visitor = visitor.enter_unique(DATA_NAME, mem::size_of::<*const T>());
            {
                let mut visitor = visitor.enter(
                    CAPACITY_NAME,
                    hashbrown_util::raw_table_alloc_size_for_len::<T>(self.capacity()),
                );
                visitor.visit_iter::<T, _>(self.iter());
                visitor.exit();
            }
            visitor.exit();
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;

    use hashbrown::raw::RawTable;
    use hashbrown::HashTable;

    use crate::golden::golden_test;

    fn hash<H: Hash>(value: &H) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_raw_table() {
        let mut table = RawTable::with_capacity(100);
        for i in 0..100 {
            table.insert(hash(&i.to_string()), i.to_string(), hash);
        }

        golden_test!(&table);
    }

    #[test]
    fn test_hash_table() {
        let mut table = HashTable::with_capacity(100);
        for i in 0..100 {
            table.insert_unique(hash(&i.to_string()), i.to_string(), hash);
        }

        golden_test!(&table);
    }
}
