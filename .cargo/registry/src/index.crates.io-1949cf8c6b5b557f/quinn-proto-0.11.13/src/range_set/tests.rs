use std::ops::Range;

use super::*;

macro_rules! common_set_tests {
    ($set_name:ident, $set_type:ident) => {
        mod $set_name {
            use super::*;

            #[test]
            fn merge_and_split() {
                let mut set = $set_type::new();
                assert!(set.insert(0..2));
                assert!(set.insert(2..4));
                assert!(!set.insert(1..3));
                assert_eq!(set.len(), 1);
                assert_eq!(&set.elts().collect::<Vec<_>>()[..], [0, 1, 2, 3]);
                assert!(!set.contains(4));
                assert!(set.remove(2..3));
                assert_eq!(set.len(), 2);
                assert!(!set.contains(2));
                assert_eq!(&set.elts().collect::<Vec<_>>()[..], [0, 1, 3]);
            }

            #[test]
            fn double_merge_exact() {
                let mut set = $set_type::new();
                assert!(set.insert(0..2));
                assert!(set.insert(4..6));
                assert_eq!(set.len(), 2);
                assert!(set.insert(2..4));
                assert_eq!(set.len(), 1);
                assert_eq!(&set.elts().collect::<Vec<_>>()[..], [0, 1, 2, 3, 4, 5]);
            }

            #[test]
            fn single_merge_low() {
                let mut set = $set_type::new();
                assert!(set.insert(0..2));
                assert!(set.insert(4..6));
                assert_eq!(set.len(), 2);
                assert!(set.insert(2..3));
                assert_eq!(set.len(), 2);
                assert_eq!(&set.elts().collect::<Vec<_>>()[..], [0, 1, 2, 4, 5]);
            }

            #[test]
            fn single_merge_high() {
                let mut set = $set_type::new();
                assert!(set.insert(0..2));
                assert!(set.insert(4..6));
                assert_eq!(set.len(), 2);
                assert!(set.insert(3..4));
                assert_eq!(set.len(), 2);
                assert_eq!(&set.elts().collect::<Vec<_>>()[..], [0, 1, 3, 4, 5]);
            }

            #[test]
            fn double_merge_wide() {
                let mut set = $set_type::new();
                assert!(set.insert(0..2));
                assert!(set.insert(4..6));
                assert_eq!(set.len(), 2);
                assert!(set.insert(1..5));
                assert_eq!(set.len(), 1);
                assert_eq!(&set.elts().collect::<Vec<_>>()[..], [0, 1, 2, 3, 4, 5]);
            }

            #[test]
            fn double_remove() {
                let mut set = $set_type::new();
                assert!(set.insert(0..2));
                assert!(set.insert(4..6));
                assert!(set.remove(1..5));
                assert_eq!(set.len(), 2);
                assert_eq!(&set.elts().collect::<Vec<_>>()[..], [0, 5]);
            }

            #[test]
            fn insert_multiple() {
                let mut set = $set_type::new();
                assert!(set.insert(0..1));
                assert!(set.insert(2..3));
                assert!(set.insert(4..5));
                assert!(set.insert(0..5));
                assert_eq!(set.len(), 1);
            }

            #[test]
            fn remove_multiple() {
                let mut set = $set_type::new();
                assert!(set.insert(0..1));
                assert!(set.insert(2..3));
                assert!(set.insert(4..5));
                assert!(set.remove(0..5));
                assert!(set.is_empty());
            }

            #[test]
            fn double_insert() {
                let mut set = $set_type::new();
                assert!(set.insert(0..2));
                assert!(!set.insert(0..2));
                assert!(set.insert(2..4));
                assert!(!set.insert(2..4));
                assert!(!set.insert(0..4));
                assert!(!set.insert(1..2));
                assert!(!set.insert(1..3));
                assert!(!set.insert(1..4));
                assert_eq!(set.len(), 1);
            }

            #[test]
            fn skip_empty_ranges() {
                let mut set = $set_type::new();
                assert!(!set.insert(2..2));
                assert_eq!(set.len(), 0);
                assert!(!set.insert(4..4));
                assert_eq!(set.len(), 0);
                assert!(!set.insert(0..0));
                assert_eq!(set.len(), 0);
            }

            #[test]
            fn compare_insert_to_reference() {
                const MAX_RANGE: u64 = 50;

                for start in 0..=MAX_RANGE {
                    for end in 0..=MAX_RANGE {
                        println!("insert({}..{})", start, end);
                        let (mut set, mut reference) = create_initial_sets(MAX_RANGE);
                        assert_eq!(set.insert(start..end), reference.insert(start..end));
                        assert_sets_equal(&set, &reference);
                    }
                }
            }

            #[test]
            fn compare_remove_to_reference() {
                const MAX_RANGE: u64 = 50;

                for start in 0..=MAX_RANGE {
                    for end in 0..=MAX_RANGE {
                        println!("remove({}..{})", start, end);
                        let (mut set, mut reference) = create_initial_sets(MAX_RANGE);
                        assert_eq!(set.remove(start..end), reference.remove(start..end));
                        assert_sets_equal(&set, &reference);
                    }
                }
            }

            #[test]
            fn min_max() {
                let mut set = $set_type::new();
                set.insert(1..3);
                set.insert(4..5);
                set.insert(6..10);
                assert_eq!(set.min(), Some(1));
                assert_eq!(set.max(), Some(9));
            }

            fn create_initial_sets(max_range: u64) -> ($set_type, RefRangeSet) {
                let mut set = $set_type::new();
                let mut reference = RefRangeSet::new(max_range as usize);
                assert_sets_equal(&set, &reference);

                assert_eq!(set.insert(2..6), reference.insert(2..6));
                assert_eq!(set.insert(10..14), reference.insert(10..14));
                assert_eq!(set.insert(14..14), reference.insert(14..14));
                assert_eq!(set.insert(18..19), reference.insert(18..19));
                assert_eq!(set.insert(20..21), reference.insert(20..21));
                assert_eq!(set.insert(22..24), reference.insert(22..24));
                assert_eq!(set.insert(26..30), reference.insert(26..30));
                assert_eq!(set.insert(34..38), reference.insert(34..38));
                assert_eq!(set.insert(42..44), reference.insert(42..44));

                assert_sets_equal(&set, &reference);

                (set, reference)
            }

            fn assert_sets_equal(set: &$set_type, reference: &RefRangeSet) {
                assert_eq!(set.len(), reference.len());
                assert_eq!(set.is_empty(), reference.is_empty());
                assert_eq!(set.elts().collect::<Vec<_>>()[..], reference.elts()[..]);
            }
        }
    };
}

common_set_tests!(range_set, RangeSet);
common_set_tests!(array_range_set, ArrayRangeSet);

/// A very simple reference implementation of a RangeSet
struct RefRangeSet {
    data: Vec<bool>,
}

impl RefRangeSet {
    fn new(capacity: usize) -> Self {
        Self {
            data: vec![false; capacity],
        }
    }

    fn len(&self) -> usize {
        let mut last = false;
        let mut count = 0;

        for v in self.data.iter() {
            if !last && *v {
                count += 1;
            }
            last = *v;
        }

        count
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn insert(&mut self, x: Range<u64>) -> bool {
        let mut result = false;

        assert!(x.end <= self.data.len() as u64);

        for i in x {
            let i = i as usize;
            if !self.data[i] {
                result = true;
                self.data[i] = true;
            }
        }

        result
    }

    fn remove(&mut self, x: Range<u64>) -> bool {
        let mut result = false;

        assert!(x.end <= self.data.len() as u64);

        for i in x {
            let i = i as usize;
            if self.data[i] {
                result = true;
                self.data[i] = false;
            }
        }

        result
    }

    fn elts(&self) -> Vec<u64> {
        self.data
            .iter()
            .enumerate()
            .filter_map(|(i, e)| if *e { Some(i as u64) } else { None })
            .collect()
    }
}
