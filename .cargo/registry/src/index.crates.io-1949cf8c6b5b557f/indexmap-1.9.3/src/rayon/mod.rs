use rayon::prelude::*;

use alloc::collections::LinkedList;

use crate::vec::Vec;

pub mod map;
pub mod set;

// This form of intermediate collection is also how Rayon collects `HashMap`.
// Note that the order will also be preserved!
fn collect<I: IntoParallelIterator>(iter: I) -> LinkedList<Vec<I::Item>> {
    iter.into_par_iter()
        .fold(Vec::new, |mut vec, elem| {
            vec.push(elem);
            vec
        })
        .map(|vec| {
            let mut list = LinkedList::new();
            list.push_back(vec);
            list
        })
        .reduce(LinkedList::new, |mut list1, mut list2| {
            list1.append(&mut list2);
            list1
        })
}
