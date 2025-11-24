use std::collections::HashMap;
use std::hash::Hash;

use super::DiffableStrRef;

// quick and dirty way to get an upper sequence ratio.
pub fn upper_seq_ratio<T: PartialEq>(seq1: &[T], seq2: &[T]) -> f32 {
    let n = seq1.len() + seq2.len();
    if n == 0 {
        1.0
    } else {
        2.0 * seq1.len().min(seq2.len()) as f32 / n as f32
    }
}

/// Internal utility to calculate an upper bound for a ratio for
/// [`get_close_matches`].  This is based on Python's difflib approach
/// of considering the two sets to be multisets.
///
/// It counts the number of matches without regard to order, which is an
/// obvious upper bound.
pub struct QuickSeqRatio<'a, T: DiffableStrRef + ?Sized>(HashMap<&'a T, i32>);

impl<'a, T: DiffableStrRef + Hash + Eq + ?Sized> QuickSeqRatio<'a, T> {
    pub fn new(seq: &[&'a T]) -> QuickSeqRatio<'a, T> {
        let mut counts = HashMap::new();
        for &word in seq {
            *counts.entry(word).or_insert(0) += 1;
        }
        QuickSeqRatio(counts)
    }

    pub fn calc(&self, seq: &[&T]) -> f32 {
        let n = self.0.len() + seq.len();
        if n == 0 {
            return 1.0;
        }

        let mut available = HashMap::new();
        let mut matches = 0;
        for &word in seq {
            let x = if let Some(count) = available.get(&word) {
                *count
            } else {
                self.0.get(&word).copied().unwrap_or(0)
            };
            available.insert(word, x - 1);
            if x > 0 {
                matches += 1;
            }
        }

        2.0 * matches as f32 / n as f32
    }
}
