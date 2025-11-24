use crate::collections::{map, Map};
use std::collections::VecDeque;
use std::fmt::Debug;
use std::hash::Hash;

pub struct KernelSet<K: Kernel> {
    counter: usize,
    kernels: VecDeque<K>,
    map: Map<K, K::Index>,
}

pub trait Kernel: Clone + Debug + Hash + Eq + PartialOrd + Ord {
    type Index: Copy + Debug;

    fn index(c: usize) -> Self::Index;
}

impl<K: Kernel> KernelSet<K> {
    pub fn new() -> KernelSet<K> {
        KernelSet {
            kernels: VecDeque::new(),
            map: map(),
            counter: 0,
        }
    }

    pub fn add_state(&mut self, kernel: K) -> K::Index {
        let kernels = &mut self.kernels;
        let counter = &mut self.counter;
        *self.map.entry(kernel.clone()).or_insert_with(|| {
            let index = *counter;
            *counter += 1;
            kernels.push_back(kernel);
            K::index(index)
        })
    }

    pub fn next(&mut self) -> Option<K> {
        self.kernels.pop_front()
    }
}
