//! When we are combining two NFAs, we will grab all the outgoing
//! edges from a set of nodes and wind up with a bunch of potentially
//! overlapping character ranges like:
//!
//!     a-z
//!     c-l
//!     0-9
//!
//! This module contains code to turn those into non-overlapping ranges like:
//!
//!     a-b
//!     c-l
//!     m-z
//!     0-9
//!
//! Specifically, we want to ensure that the same set of characters is
//! covered when we started, and that each of the input ranges is
//! covered precisely by some set of ranges in the output.

use crate::collections::Set;
use crate::lexer::nfa::Test;
use std::cmp;

pub fn remove_overlap(ranges: &Set<Test>) -> Vec<Test> {
    // We will do this in the dumbest possible way to start. :)
    // Maintain a result vector that contains disjoint ranges.  To
    // insert a new range, we walk over this vector and split things
    // up as we go. This algorithm is so naive as to be exponential, I
    // think. Sue me.

    let mut disjoint_ranges = vec![];

    for &range in ranges {
        add_range(range, 0, &mut disjoint_ranges);
    }

    // the algorithm above leaves some empty ranges in for simplicity;
    // prune them out.
    disjoint_ranges.retain(|r| !r.is_empty());

    disjoint_ranges
}

fn add_range(range: Test, start_index: usize, disjoint_ranges: &mut Vec<Test>) {
    if range.is_empty() {
        return;
    }

    // Find first overlapping range in `disjoint_ranges`, if any.
    match disjoint_ranges[start_index..]
        .iter()
        .position(|r| r.intersects(range))
    {
        Some(index) => {
            let index = index + start_index;
            let overlapping_range = disjoint_ranges[index];

            // If the range we are trying to add already exists, we're all done.
            if overlapping_range == range {
                return;
            }

            // Otherwise, we want to create three ranges (some of which may
            // be empty). e.g. imagine one range is `a-z` and the other
            // is `c-l`, we want `a-b`, `c-l`, and `m-z`.
            let min_min = cmp::min(range.start, overlapping_range.start);
            let mid_min = cmp::max(range.start, overlapping_range.start);
            let mid_max = cmp::min(range.end, overlapping_range.end);
            let max_max = cmp::max(range.end, overlapping_range.end);
            let low_range = Test {
                start: min_min,
                end: mid_min,
            };
            let mid_range = Test {
                start: mid_min,
                end: mid_max,
            };
            let max_range = Test {
                start: mid_max,
                end: max_max,
            };

            assert!(low_range.is_disjoint(mid_range));
            assert!(low_range.is_disjoint(max_range));
            assert!(mid_range.is_disjoint(max_range));

            // Replace the existing range with the low range, and then
            // add the mid and max ranges in. (The low range may be
            // empty, but we'll prune that out later.)
            disjoint_ranges[index] = low_range;
            add_range(mid_range, index + 1, disjoint_ranges);
            add_range(max_range, index + 1, disjoint_ranges);
        }

        None => {
            // no overlap -- easy case.
            disjoint_ranges.push(range);
        }
    }
}

#[cfg(test)]
macro_rules! test {
    ($($range:expr,)*) => {
        {
            use crate::collections::set;
            use crate::lexer::nfa::Test;
            use std::char;
            let mut s = set();
            $({ let r = $range; s.insert(Test::exclusive_range(r.start, r.end)); })*
            remove_overlap(&s).into_iter()
                              .map(|r|
                                   char::from_u32(r.start).unwrap() ..
                                   char::from_u32(r.end).unwrap())
                              .collect::<Vec<_>>()
        }
    }
}

#[test]
fn alphabet() {
    let result = test! {
        'a' .. 'z',
        'c' .. 'l',
        '0' .. '9',
    };
    assert_eq!(result, vec!['0'..'9', 'a'..'c', 'c'..'l', 'l'..'z']);
}

#[test]
fn repeat() {
    let result = test! {
        'a' .. 'z',
        'c' .. 'l',
        'l' .. 'z',
        '0' .. '9',
    };
    assert_eq!(result, vec!['0'..'9', 'a'..'c', 'c'..'l', 'l'..'z']);
}

#[test]
fn stagger() {
    let result = test! {
        '0' .. '3',
        '2' .. '4',
        '3' .. '5',
    };
    assert_eq!(result, vec!['0'..'2', '2'..'3', '3'..'4', '4'..'5']);
}
