#![cfg(feature = "rayon")]

#[macro_use]
extern crate lazy_static;

use hashbrown::{HashMap, HashSet};
use rayon::iter::{
    IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelExtend,
    ParallelIterator,
};

macro_rules! assert_eq3 {
    ($e1:expr, $e2:expr, $e3:expr) => {{
        assert_eq!($e1, $e2);
        assert_eq!($e1, $e3);
        assert_eq!($e2, $e3);
    }};
}

lazy_static! {
    static ref MAP_EMPTY: HashMap<char, u32> = HashMap::new();
    static ref MAP: HashMap<char, u32> = {
        let mut m = HashMap::new();
        m.insert('b', 20);
        m.insert('a', 10);
        m.insert('c', 30);
        m.insert('e', 50);
        m.insert('f', 60);
        m.insert('d', 40);
        m
    };
}

#[test]
fn map_seq_par_equivalence_iter_empty() {
    let vec_seq = MAP_EMPTY.iter().collect::<Vec<_>>();
    let vec_par = MAP_EMPTY.par_iter().collect::<Vec<_>>();

    assert_eq3!(vec_seq, vec_par, []);
}

#[test]
fn map_seq_par_equivalence_iter() {
    let mut vec_seq = MAP.iter().collect::<Vec<_>>();
    let mut vec_par = MAP.par_iter().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = [
        (&'a', &10),
        (&'b', &20),
        (&'c', &30),
        (&'d', &40),
        (&'e', &50),
        (&'f', &60),
    ];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

#[test]
fn map_seq_par_equivalence_keys_empty() {
    let vec_seq = MAP_EMPTY.keys().collect::<Vec<&char>>();
    let vec_par = MAP_EMPTY.par_keys().collect::<Vec<&char>>();

    let expected: [&char; 0] = [];

    assert_eq3!(vec_seq, vec_par, expected);
}

#[test]
fn map_seq_par_equivalence_keys() {
    let mut vec_seq = MAP.keys().collect::<Vec<_>>();
    let mut vec_par = MAP.par_keys().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = [&'a', &'b', &'c', &'d', &'e', &'f'];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

#[test]
fn map_seq_par_equivalence_values_empty() {
    let vec_seq = MAP_EMPTY.values().collect::<Vec<_>>();
    let vec_par = MAP_EMPTY.par_values().collect::<Vec<_>>();

    let expected: [&u32; 0] = [];

    assert_eq3!(vec_seq, vec_par, expected);
}

#[test]
fn map_seq_par_equivalence_values() {
    let mut vec_seq = MAP.values().collect::<Vec<_>>();
    let mut vec_par = MAP.par_values().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = [&10, &20, &30, &40, &50, &60];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

#[test]
fn map_seq_par_equivalence_iter_mut_empty() {
    let mut map1 = MAP_EMPTY.clone();
    let mut map2 = MAP_EMPTY.clone();

    let vec_seq = map1.iter_mut().collect::<Vec<_>>();
    let vec_par = map2.par_iter_mut().collect::<Vec<_>>();

    assert_eq3!(vec_seq, vec_par, []);
}

#[test]
fn map_seq_par_equivalence_iter_mut() {
    let mut map1 = MAP.clone();
    let mut map2 = MAP.clone();

    let mut vec_seq = map1.iter_mut().collect::<Vec<_>>();
    let mut vec_par = map2.par_iter_mut().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = [
        (&'a', &mut 10),
        (&'b', &mut 20),
        (&'c', &mut 30),
        (&'d', &mut 40),
        (&'e', &mut 50),
        (&'f', &mut 60),
    ];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

#[test]
fn map_seq_par_equivalence_values_mut_empty() {
    let mut map1 = MAP_EMPTY.clone();
    let mut map2 = MAP_EMPTY.clone();

    let vec_seq = map1.values_mut().collect::<Vec<_>>();
    let vec_par = map2.par_values_mut().collect::<Vec<_>>();

    let expected: [&u32; 0] = [];

    assert_eq3!(vec_seq, vec_par, expected);
}

#[test]
fn map_seq_par_equivalence_values_mut() {
    let mut map1 = MAP.clone();
    let mut map2 = MAP.clone();

    let mut vec_seq = map1.values_mut().collect::<Vec<_>>();
    let mut vec_par = map2.par_values_mut().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = [&mut 10, &mut 20, &mut 30, &mut 40, &mut 50, &mut 60];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

#[test]
fn map_seq_par_equivalence_into_iter_empty() {
    let vec_seq = MAP_EMPTY.clone().into_iter().collect::<Vec<_>>();
    let vec_par = MAP_EMPTY.clone().into_par_iter().collect::<Vec<_>>();

    assert_eq3!(vec_seq, vec_par, []);
}

#[test]
fn map_seq_par_equivalence_into_iter() {
    let mut vec_seq = MAP.clone().into_iter().collect::<Vec<_>>();
    let mut vec_par = MAP.clone().into_par_iter().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = [
        ('a', 10),
        ('b', 20),
        ('c', 30),
        ('d', 40),
        ('e', 50),
        ('f', 60),
    ];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

lazy_static! {
    static ref MAP_VEC_EMPTY: Vec<(char, u32)> = vec![];
    static ref MAP_VEC: Vec<(char, u32)> = vec![
        ('b', 20),
        ('a', 10),
        ('c', 30),
        ('e', 50),
        ('f', 60),
        ('d', 40),
    ];
}

#[test]
fn map_seq_par_equivalence_collect_empty() {
    let map_expected = MAP_EMPTY.clone();
    let map_seq = MAP_VEC_EMPTY.clone().into_iter().collect::<HashMap<_, _>>();
    let map_par = MAP_VEC_EMPTY
        .clone()
        .into_par_iter()
        .collect::<HashMap<_, _>>();

    assert_eq!(map_seq, map_par);
    assert_eq!(map_seq, map_expected);
    assert_eq!(map_par, map_expected);
}

#[test]
fn map_seq_par_equivalence_collect() {
    let map_expected = MAP.clone();
    let map_seq = MAP_VEC.clone().into_iter().collect::<HashMap<_, _>>();
    let map_par = MAP_VEC.clone().into_par_iter().collect::<HashMap<_, _>>();

    assert_eq!(map_seq, map_par);
    assert_eq!(map_seq, map_expected);
    assert_eq!(map_par, map_expected);
}

lazy_static! {
    static ref MAP_EXISTING_EMPTY: HashMap<char, u32> = HashMap::new();
    static ref MAP_EXISTING: HashMap<char, u32> = {
        let mut m = HashMap::new();
        m.insert('b', 20);
        m.insert('a', 10);
        m
    };
    static ref MAP_EXTENSION_EMPTY: Vec<(char, u32)> = vec![];
    static ref MAP_EXTENSION: Vec<(char, u32)> = vec![('c', 30), ('e', 50), ('f', 60), ('d', 40),];
}

#[test]
fn map_seq_par_equivalence_existing_empty_extend_empty() {
    let expected = HashMap::new();
    let mut map_seq = MAP_EXISTING_EMPTY.clone();
    let mut map_par = MAP_EXISTING_EMPTY.clone();

    map_seq.extend(MAP_EXTENSION_EMPTY.iter().copied());
    map_par.par_extend(MAP_EXTENSION_EMPTY.par_iter().copied());

    assert_eq3!(map_seq, map_par, expected);
}

#[test]
fn map_seq_par_equivalence_existing_empty_extend() {
    let expected = MAP_EXTENSION.iter().copied().collect::<HashMap<_, _>>();
    let mut map_seq = MAP_EXISTING_EMPTY.clone();
    let mut map_par = MAP_EXISTING_EMPTY.clone();

    map_seq.extend(MAP_EXTENSION.iter().copied());
    map_par.par_extend(MAP_EXTENSION.par_iter().copied());

    assert_eq3!(map_seq, map_par, expected);
}

#[test]
fn map_seq_par_equivalence_existing_extend_empty() {
    let expected = MAP_EXISTING.clone();
    let mut map_seq = MAP_EXISTING.clone();
    let mut map_par = MAP_EXISTING.clone();

    map_seq.extend(MAP_EXTENSION_EMPTY.iter().copied());
    map_par.par_extend(MAP_EXTENSION_EMPTY.par_iter().copied());

    assert_eq3!(map_seq, map_par, expected);
}

#[test]
fn map_seq_par_equivalence_existing_extend() {
    let expected = MAP.clone();
    let mut map_seq = MAP_EXISTING.clone();
    let mut map_par = MAP_EXISTING.clone();

    map_seq.extend(MAP_EXTENSION.iter().copied());
    map_par.par_extend(MAP_EXTENSION.par_iter().copied());

    assert_eq3!(map_seq, map_par, expected);
}

lazy_static! {
    static ref SET_EMPTY: HashSet<char> = HashSet::new();
    static ref SET: HashSet<char> = {
        let mut s = HashSet::new();
        s.insert('b');
        s.insert('a');
        s.insert('c');
        s.insert('e');
        s.insert('f');
        s.insert('d');
        s
    };
}

#[test]
fn set_seq_par_equivalence_iter_empty() {
    let vec_seq = SET_EMPTY.iter().collect::<Vec<_>>();
    let vec_par = SET_EMPTY.par_iter().collect::<Vec<_>>();

    let expected: [&char; 0] = [];

    assert_eq3!(vec_seq, vec_par, expected);
}

#[test]
fn set_seq_par_equivalence_iter() {
    let mut vec_seq = SET.iter().collect::<Vec<_>>();
    let mut vec_par = SET.par_iter().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = [&'a', &'b', &'c', &'d', &'e', &'f'];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

#[test]
fn set_seq_par_equivalence_into_iter_empty() {
    let vec_seq = SET_EMPTY.clone().into_iter().collect::<Vec<_>>();
    let vec_par = SET_EMPTY.clone().into_par_iter().collect::<Vec<_>>();

    // Work around type inference failure introduced by rend dev-dependency.
    let empty: [char; 0] = [];
    assert_eq3!(vec_seq, vec_par, empty);
}

#[test]
fn set_seq_par_equivalence_into_iter() {
    let mut vec_seq = SET.clone().into_iter().collect::<Vec<_>>();
    let mut vec_par = SET.clone().into_par_iter().collect::<Vec<_>>();

    assert_eq!(vec_seq, vec_par);

    // Do not depend on the exact order of values
    let expected_sorted = ['a', 'b', 'c', 'd', 'e', 'f'];

    vec_seq.sort_unstable();
    vec_par.sort_unstable();

    assert_eq3!(vec_seq, vec_par, expected_sorted);
}

lazy_static! {
    static ref SET_VEC_EMPTY: Vec<char> = vec![];
    static ref SET_VEC: Vec<char> = vec!['b', 'a', 'c', 'e', 'f', 'd',];
}

#[test]
fn set_seq_par_equivalence_collect_empty() {
    let set_expected = SET_EMPTY.clone();
    let set_seq = SET_VEC_EMPTY.clone().into_iter().collect::<HashSet<_>>();
    let set_par = SET_VEC_EMPTY
        .clone()
        .into_par_iter()
        .collect::<HashSet<_>>();

    assert_eq!(set_seq, set_par);
    assert_eq!(set_seq, set_expected);
    assert_eq!(set_par, set_expected);
}

#[test]
fn set_seq_par_equivalence_collect() {
    let set_expected = SET.clone();
    let set_seq = SET_VEC.clone().into_iter().collect::<HashSet<_>>();
    let set_par = SET_VEC.clone().into_par_iter().collect::<HashSet<_>>();

    assert_eq!(set_seq, set_par);
    assert_eq!(set_seq, set_expected);
    assert_eq!(set_par, set_expected);
}

lazy_static! {
    static ref SET_EXISTING_EMPTY: HashSet<char> = HashSet::new();
    static ref SET_EXISTING: HashSet<char> = {
        let mut s = HashSet::new();
        s.insert('b');
        s.insert('a');
        s
    };
    static ref SET_EXTENSION_EMPTY: Vec<char> = vec![];
    static ref SET_EXTENSION: Vec<char> = vec!['c', 'e', 'f', 'd',];
}

#[test]
fn set_seq_par_equivalence_existing_empty_extend_empty() {
    let expected = HashSet::new();
    let mut set_seq = SET_EXISTING_EMPTY.clone();
    let mut set_par = SET_EXISTING_EMPTY.clone();

    set_seq.extend(SET_EXTENSION_EMPTY.iter().copied());
    set_par.par_extend(SET_EXTENSION_EMPTY.par_iter().copied());

    assert_eq3!(set_seq, set_par, expected);
}

#[test]
fn set_seq_par_equivalence_existing_empty_extend() {
    let expected = SET_EXTENSION.iter().copied().collect::<HashSet<_>>();
    let mut set_seq = SET_EXISTING_EMPTY.clone();
    let mut set_par = SET_EXISTING_EMPTY.clone();

    set_seq.extend(SET_EXTENSION.iter().copied());
    set_par.par_extend(SET_EXTENSION.par_iter().copied());

    assert_eq3!(set_seq, set_par, expected);
}

#[test]
fn set_seq_par_equivalence_existing_extend_empty() {
    let expected = SET_EXISTING.clone();
    let mut set_seq = SET_EXISTING.clone();
    let mut set_par = SET_EXISTING.clone();

    set_seq.extend(SET_EXTENSION_EMPTY.iter().copied());
    set_par.par_extend(SET_EXTENSION_EMPTY.par_iter().copied());

    assert_eq3!(set_seq, set_par, expected);
}

#[test]
fn set_seq_par_equivalence_existing_extend() {
    let expected = SET.clone();
    let mut set_seq = SET_EXISTING.clone();
    let mut set_par = SET_EXISTING.clone();

    set_seq.extend(SET_EXTENSION.iter().copied());
    set_par.par_extend(SET_EXTENSION.par_iter().copied());

    assert_eq3!(set_seq, set_par, expected);
}

lazy_static! {
    static ref SET_A: HashSet<char> = ['a', 'b', 'c', 'd'].iter().copied().collect();
    static ref SET_B: HashSet<char> = ['a', 'b', 'e', 'f'].iter().copied().collect();
    static ref SET_DIFF_AB: HashSet<char> = ['c', 'd'].iter().copied().collect();
    static ref SET_DIFF_BA: HashSet<char> = ['e', 'f'].iter().copied().collect();
    static ref SET_SYMM_DIFF_AB: HashSet<char> = ['c', 'd', 'e', 'f'].iter().copied().collect();
    static ref SET_INTERSECTION_AB: HashSet<char> = ['a', 'b'].iter().copied().collect();
    static ref SET_UNION_AB: HashSet<char> =
        ['a', 'b', 'c', 'd', 'e', 'f'].iter().copied().collect();
}

#[test]
fn set_seq_par_equivalence_difference() {
    let diff_ab_seq = SET_A.difference(&*SET_B).copied().collect::<HashSet<_>>();
    let diff_ab_par = SET_A
        .par_difference(&*SET_B)
        .copied()
        .collect::<HashSet<_>>();

    assert_eq3!(diff_ab_seq, diff_ab_par, *SET_DIFF_AB);

    let diff_ba_seq = SET_B.difference(&*SET_A).copied().collect::<HashSet<_>>();
    let diff_ba_par = SET_B
        .par_difference(&*SET_A)
        .copied()
        .collect::<HashSet<_>>();

    assert_eq3!(diff_ba_seq, diff_ba_par, *SET_DIFF_BA);
}

#[test]
fn set_seq_par_equivalence_symmetric_difference() {
    let symm_diff_ab_seq = SET_A
        .symmetric_difference(&*SET_B)
        .copied()
        .collect::<HashSet<_>>();
    let symm_diff_ab_par = SET_A
        .par_symmetric_difference(&*SET_B)
        .copied()
        .collect::<HashSet<_>>();

    assert_eq3!(symm_diff_ab_seq, symm_diff_ab_par, *SET_SYMM_DIFF_AB);
}

#[test]
fn set_seq_par_equivalence_intersection() {
    let intersection_ab_seq = SET_A.intersection(&*SET_B).copied().collect::<HashSet<_>>();
    let intersection_ab_par = SET_A
        .par_intersection(&*SET_B)
        .copied()
        .collect::<HashSet<_>>();

    assert_eq3!(
        intersection_ab_seq,
        intersection_ab_par,
        *SET_INTERSECTION_AB
    );
}

#[test]
fn set_seq_par_equivalence_union() {
    let union_ab_seq = SET_A.union(&*SET_B).copied().collect::<HashSet<_>>();
    let union_ab_par = SET_A.par_union(&*SET_B).copied().collect::<HashSet<_>>();

    assert_eq3!(union_ab_seq, union_ab_par, *SET_UNION_AB);
}
