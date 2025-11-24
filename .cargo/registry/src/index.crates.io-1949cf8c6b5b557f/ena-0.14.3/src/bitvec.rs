// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// A very simple BitVector type.
pub struct BitVector {
    data: Vec<u64>,
}

impl BitVector {
    pub fn new(num_bits: usize) -> BitVector {
        let num_words = u64s(num_bits);
        BitVector { data: vec![0; num_words] }
    }

    pub fn contains(&self, bit: usize) -> bool {
        let (word, mask) = word_mask(bit);
        (self.data[word] & mask) != 0
    }

    /// Returns true if the bit has changed.
    pub fn insert(&mut self, bit: usize) -> bool {
        let (word, mask) = word_mask(bit);
        let data = &mut self.data[word];
        let value = *data;
        let new_value = value | mask;
        *data = new_value;
        new_value != value
    }

    pub fn insert_all(&mut self, all: &BitVector) -> bool {
        assert!(self.data.len() == all.data.len());
        let mut changed = false;
        for (i, j) in self.data.iter_mut().zip(&all.data) {
            let value = *i;
            *i = value | *j;
            if value != *i {
                changed = true;
            }
        }
        changed
    }

    pub fn grow(&mut self, num_bits: usize) {
        let num_words = u64s(num_bits);
        let extra_words = self.data.len() - num_words;
        self.data.extend((0..extra_words).map(|_| 0));
    }

    /// Iterates over indexes of set bits in a sorted order
    pub fn iter<'a>(&'a self) -> BitVectorIter<'a> {
        BitVectorIter {
            iter: self.data.iter(),
            current: 0,
            idx: 0,
        }
    }
}

pub struct BitVectorIter<'a> {
    iter: ::std::slice::Iter<'a, u64>,
    current: u64,
    idx: usize,
}

impl<'a> Iterator for BitVectorIter<'a> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        while self.current == 0 {
            self.current = if let Some(&i) = self.iter.next() {
                if i == 0 {
                    self.idx += 64;
                    continue;
                } else {
                    self.idx = u64s(self.idx) * 64;
                    i
                }
            } else {
                return None;
            }
        }
        let offset = self.current.trailing_zeros() as usize;
        self.current >>= offset;
        self.current >>= 1; // shift otherwise overflows for 0b1000_0000_…_0000
        self.idx += offset + 1;
        return Some(self.idx - 1);
    }
}

/// A "bit matrix" is basically a square matrix of booleans
/// represented as one gigantic bitvector. In other words, it is as if
/// you have N bitvectors, each of length N. Note that `elements` here is `N`/
#[derive(Clone)]
pub struct BitMatrix {
    elements: usize,
    vector: Vec<u64>,
}

impl BitMatrix {
    // Create a new `elements x elements` matrix, initially empty.
    pub fn new(elements: usize) -> BitMatrix {
        // For every element, we need one bit for every other
        // element. Round up to an even number of u64s.
        let u64s_per_elem = u64s(elements);
        BitMatrix {
            elements: elements,
            vector: vec![0; elements * u64s_per_elem],
        }
    }

    /// The range of bits for a given element.
    fn range(&self, element: usize) -> (usize, usize) {
        let u64s_per_elem = u64s(self.elements);
        let start = element * u64s_per_elem;
        (start, start + u64s_per_elem)
    }

    pub fn add(&mut self, source: usize, target: usize) -> bool {
        let (start, _) = self.range(source);
        let (word, mask) = word_mask(target);
        let mut vector = &mut self.vector[..];
        let v1 = vector[start + word];
        let v2 = v1 | mask;
        vector[start + word] = v2;
        v1 != v2
    }

    /// Do the bits from `source` contain `target`?
    ///
    /// Put another way, if the matrix represents (transitive)
    /// reachability, can `source` reach `target`?
    pub fn contains(&self, source: usize, target: usize) -> bool {
        let (start, _) = self.range(source);
        let (word, mask) = word_mask(target);
        (self.vector[start + word] & mask) != 0
    }

    /// Returns those indices that are reachable from both `a` and
    /// `b`. This is an O(n) operation where `n` is the number of
    /// elements (somewhat independent from the actual size of the
    /// intersection, in particular).
    pub fn intersection(&self, a: usize, b: usize) -> Vec<usize> {
        let (a_start, a_end) = self.range(a);
        let (b_start, b_end) = self.range(b);
        let mut result = Vec::with_capacity(self.elements);
        for (base, (i, j)) in (a_start..a_end).zip(b_start..b_end).enumerate() {
            let mut v = self.vector[i] & self.vector[j];
            for bit in 0..64 {
                if v == 0 {
                    break;
                }
                if v & 0x1 != 0 {
                    result.push(base * 64 + bit);
                }
                v >>= 1;
            }
        }
        result
    }

    /// Add the bits from `read` to the bits from `write`,
    /// return true if anything changed.
    ///
    /// This is used when computing transitive reachability because if
    /// you have an edge `write -> read`, because in that case
    /// `write` can reach everything that `read` can (and
    /// potentially more).
    pub fn merge(&mut self, read: usize, write: usize) -> bool {
        let (read_start, read_end) = self.range(read);
        let (write_start, write_end) = self.range(write);
        let vector = &mut self.vector[..];
        let mut changed = false;
        for (read_index, write_index) in (read_start..read_end).zip(write_start..write_end) {
            let v1 = vector[write_index];
            let v2 = v1 | vector[read_index];
            vector[write_index] = v2;
            changed = changed | (v1 != v2);
        }
        changed
    }
}

fn u64s(elements: usize) -> usize {
    (elements + 63) / 64
}

fn word_mask(index: usize) -> (usize, u64) {
    let word = index / 64;
    let mask = 1 << (index % 64);
    (word, mask)
}

#[test]
fn bitvec_iter_works() {
    let mut bitvec = BitVector::new(100);
    bitvec.insert(1);
    bitvec.insert(10);
    bitvec.insert(19);
    bitvec.insert(62);
    bitvec.insert(63);
    bitvec.insert(64);
    bitvec.insert(65);
    bitvec.insert(66);
    bitvec.insert(99);
    assert_eq!(bitvec.iter().collect::<Vec<_>>(),
               [1, 10, 19, 62, 63, 64, 65, 66, 99]);
}

#[test]
fn bitvec_iter_works_2() {
    let mut bitvec = BitVector::new(300);
    bitvec.insert(1);
    bitvec.insert(10);
    bitvec.insert(19);
    bitvec.insert(62);
    bitvec.insert(66);
    bitvec.insert(99);
    bitvec.insert(299);
    assert_eq!(bitvec.iter().collect::<Vec<_>>(),
               [1, 10, 19, 62, 66, 99, 299]);

}

#[test]
fn bitvec_iter_works_3() {
    let mut bitvec = BitVector::new(319);
    bitvec.insert(0);
    bitvec.insert(127);
    bitvec.insert(191);
    bitvec.insert(255);
    bitvec.insert(319);
    assert_eq!(bitvec.iter().collect::<Vec<_>>(), [0, 127, 191, 255, 319]);
}

#[test]
fn union_two_vecs() {
    let mut vec1 = BitVector::new(65);
    let mut vec2 = BitVector::new(65);
    assert!(vec1.insert(3));
    assert!(!vec1.insert(3));
    assert!(vec2.insert(5));
    assert!(vec2.insert(64));
    assert!(vec1.insert_all(&vec2));
    assert!(!vec1.insert_all(&vec2));
    assert!(vec1.contains(3));
    assert!(!vec1.contains(4));
    assert!(vec1.contains(5));
    assert!(!vec1.contains(63));
    assert!(vec1.contains(64));
}

#[test]
fn grow() {
    let mut vec1 = BitVector::new(65);
    assert!(vec1.insert(3));
    assert!(!vec1.insert(3));
    assert!(vec1.insert(5));
    assert!(vec1.insert(64));
    vec1.grow(128);
    assert!(vec1.contains(3));
    assert!(vec1.contains(5));
    assert!(vec1.contains(64));
    assert!(!vec1.contains(126));
}

#[test]
fn matrix_intersection() {
    let mut vec1 = BitMatrix::new(200);

    // (*) Elements reachable from both 2 and 65.

    vec1.add(2, 3);
    vec1.add(2, 6);
    vec1.add(2, 10); // (*)
    vec1.add(2, 64); // (*)
    vec1.add(2, 65);
    vec1.add(2, 130);
    vec1.add(2, 160); // (*)

    vec1.add(64, 133);

    vec1.add(65, 2);
    vec1.add(65, 8);
    vec1.add(65, 10); // (*)
    vec1.add(65, 64); // (*)
    vec1.add(65, 68);
    vec1.add(65, 133);
    vec1.add(65, 160); // (*)

    let intersection = vec1.intersection(2, 64);
    assert!(intersection.is_empty());

    let intersection = vec1.intersection(2, 65);
    assert_eq!(intersection, &[10, 64, 160]);
}
