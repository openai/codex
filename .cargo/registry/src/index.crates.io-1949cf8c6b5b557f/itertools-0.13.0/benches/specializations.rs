#![allow(unstable_name_collisions, clippy::incompatible_msrv)]

use criterion::black_box;
use criterion::BenchmarkId;
use itertools::Itertools;

const NTH_INPUTS: &[usize] = &[0, 1, 2, 4, 8];

/// Create multiple functions each defining a benchmark group about iterator methods.
///
/// Each created group has functions with the following ids:
///
/// - `next`, `size_hint`, `count`, `last`, `nth`, `collect`, `fold`
/// - and when marked as `DoubleEndedIterator`: `next_back`, `nth_back`, `rfold`
/// - and when marked as `ExactSizeIterator`: `len`
///
/// Note that this macro can be called only once.
macro_rules! bench_specializations {
    (
        $(
            $name:ident {
                $($extra:ident)*
                {$(
                    $init:stmt;
                )*}
                $iterator:expr
            }
        )*
    ) => {
        $(
            #[allow(unused_must_use)]
            fn $name(c: &mut ::criterion::Criterion) {
                let mut bench_group = c.benchmark_group(stringify!($name));
                $(
                    $init
                )*
                let bench_first_its = {
                    let mut bench_idx = 0;
                    [0; 1000].map(|_| {
                        let mut it = $iterator;
                        if bench_idx != 0 {
                            it.nth(bench_idx - 1);
                        }
                        bench_idx += 1;
                        it
                    })
                };
                bench_specializations!(@Iterator bench_group bench_first_its: $iterator);
                $(
                    bench_specializations!(@$extra bench_group bench_first_its: $iterator);
                )*
                bench_group.finish();
            }
        )*

        ::criterion::criterion_group!(benches, $($name, )*);
        ::criterion::criterion_main!(benches);
    };

    (@Iterator $group:ident $first_its:ident: $iterator:expr) => {
        $group.bench_function("next", |bencher| bencher.iter(|| {
            let mut it = $iterator;
            while let Some(x) = it.next() {
                black_box(x);
            }
        }));
        $group.bench_function("size_hint", |bencher| bencher.iter(|| {
            $first_its.iter().for_each(|it| {
                black_box(it.size_hint());
            })
        }));
        $group.bench_function("count", |bencher| bencher.iter(|| {
            $iterator.count()
        }));
        $group.bench_function("last", |bencher| bencher.iter(|| {
            $iterator.last()
        }));
        for n in NTH_INPUTS {
            $group.bench_with_input(BenchmarkId::new("nth", n), n, |bencher, n| bencher.iter(|| {
                for start in 0_usize..10 {
                    let mut it = $iterator;
                    if let Some(s) = start.checked_sub(1) {
                        black_box(it.nth(s));
                    }
                    while let Some(x) = it.nth(*n) {
                        black_box(x);
                    }
                }
            }));
        }
        $group.bench_function("collect", |bencher| bencher.iter(|| {
            $iterator.collect::<Vec<_>>()
        }));
        $group.bench_function("fold", |bencher| bencher.iter(|| {
            $iterator.fold((), |(), x| {
                black_box(x);
            })
        }));
    };

    (@DoubleEndedIterator $group:ident $_first_its:ident: $iterator:expr) => {
        $group.bench_function("next_back", |bencher| bencher.iter(|| {
            let mut it = $iterator;
            while let Some(x) = it.next_back() {
                black_box(x);
            }
        }));
        for n in NTH_INPUTS {
            $group.bench_with_input(BenchmarkId::new("nth_back", n), n, |bencher, n| bencher.iter(|| {
                for start in 0_usize..10 {
                    let mut it = $iterator;
                    if let Some(s) = start.checked_sub(1) {
                        black_box(it.nth_back(s));
                    }
                    while let Some(x) = it.nth_back(*n) {
                        black_box(x);
                    }
                }
            }));
        }
        $group.bench_function("rfold", |bencher| bencher.iter(|| {
            $iterator.rfold((), |(), x| {
                black_box(x);
            })
        }));
    };

    (@ExactSizeIterator $group:ident $first_its:ident: $_iterator:expr) => {
        $group.bench_function("len", |bencher| bencher.iter(|| {
            $first_its.iter().for_each(|it| {
                black_box(it.len());
            })
        }));
    };
}

// Usage examples:
// - For `ZipLongest::fold` only:
//     cargo bench --bench specializations zip_longest/fold
// - For `.combinations(k).nth(8)`:
//     cargo bench --bench specializations combinations./nth/8
bench_specializations! {
    interleave {
        {
            let v1 = black_box(vec![0; 1024]);
            let v2 = black_box(vec![0; 768]);
        }
        v1.iter().interleave(&v2)
    }
    interleave_shortest {
        {
            let v1 = black_box(vec![0; 1024]);
            let v2 = black_box(vec![0; 768]);
        }
        v1.iter().interleave_shortest(&v2)
    }
    batching {
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().batching(Iterator::next)
    }
    tuple_windows1 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuple_windows::<(_,)>()
    }
    tuple_windows2 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuple_windows::<(_, _)>()
    }
    tuple_windows3 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuple_windows::<(_, _, _)>()
    }
    tuple_windows4 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuple_windows::<(_, _, _, _)>()
    }
    circular_tuple_windows1 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().circular_tuple_windows::<(_,)>()
    }
    circular_tuple_windows2 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().circular_tuple_windows::<(_, _)>()
    }
    circular_tuple_windows3 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().circular_tuple_windows::<(_, _, _)>()
    }
    circular_tuple_windows4 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().circular_tuple_windows::<(_, _, _, _)>()
    }
    tuples1 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuples::<(_,)>()
    }
    tuples2 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuples::<(_, _)>()
    }
    tuples3 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuples::<(_, _, _)>()
    }
    tuples4 {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuples::<(_, _, _, _)>()
    }
    tuple_buffer {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 11]);
            // Short but the buffer can't have 12 or more elements.
        }
        {
            let mut it = v.iter().tuples::<(_, _, _, _, _, _, _, _, _, _, _, _)>();
            it.next(); // No element but it fills the buffer.
            it.into_buffer()
        }
    }
    cartesian_product {
        {
            let v = black_box(vec![0; 16]);
        }
        itertools::iproduct!(&v, &v, &v)
    }
    multi_cartesian_product {
        {
            let vs = black_box([0; 3].map(|_| vec![0; 16]));
        }
        vs.iter().multi_cartesian_product()
    }
    coalesce {
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().coalesce(|x, y| if x == y { Ok(x) } else { Err((x, y)) })
    }
    dedup {
        {
            let v = black_box((0..32).flat_map(|x| [x; 32]).collect_vec());
        }
        v.iter().dedup()
    }
    dedup_by {
        {
            let v = black_box((0..32).flat_map(|x| [x; 32]).collect_vec());
        }
        v.iter().dedup_by(PartialOrd::ge)
    }
    dedup_with_count {
        {
            let v = black_box((0..32).flat_map(|x| [x; 32]).collect_vec());
        }
        v.iter().dedup_with_count()
    }
    dedup_by_with_count {
        {
            let v = black_box((0..32).flat_map(|x| [x; 32]).collect_vec());
        }
        v.iter().dedup_by_with_count(PartialOrd::ge)
    }
    duplicates {
        DoubleEndedIterator
        {
            let v = black_box((0..32).cycle().take(1024).collect_vec());
        }
        v.iter().duplicates()
    }
    duplicates_by {
        DoubleEndedIterator
        {
            let v = black_box((0..1024).collect_vec());
        }
        v.iter().duplicates_by(|x| *x % 10)
    }
    unique {
        DoubleEndedIterator
        {
            let v = black_box((0..32).cycle().take(1024).collect_vec());
        }
        v.iter().unique()
    }
    unique_by {
        DoubleEndedIterator
        {
            let v = black_box((0..1024).collect_vec());
        }
        v.iter().unique_by(|x| *x % 50)
    }
    take_while_inclusive {
        {
            let v = black_box((0..1024).collect_vec());
        }
        v.iter().take_while_inclusive(|x| **x < 1000)
    }
    pad_using {
        DoubleEndedIterator
        ExactSizeIterator
        {
            let v = black_box((0..1024).collect_vec());
        }
        v.iter().copied().pad_using(2048, |i| 5 * i)
    }
    positions {
        DoubleEndedIterator
        {
            let v = black_box((0..1024).collect_vec());
        }
        v.iter().positions(|x| x % 5 == 0)
    }
    update {
        DoubleEndedIterator
        ExactSizeIterator
        {
            let v = black_box((0_i32..1024).collect_vec());
        }
        v.iter().copied().update(|x| *x *= 7)
    }
    tuple_combinations1 {
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().tuple_combinations::<(_,)>()
    }
    tuple_combinations2 {
        {
            let v = black_box(vec![0; 64]);
        }
        v.iter().tuple_combinations::<(_, _)>()
    }
    tuple_combinations3 {
        {
            let v = black_box(vec![0; 64]);
        }
        v.iter().tuple_combinations::<(_, _, _)>()
    }
    tuple_combinations4 {
        {
            let v = black_box(vec![0; 64]);
        }
        v.iter().tuple_combinations::<(_, _, _, _)>()
    }
    intersperse {
        {
            let v = black_box(vec![0; 1024]);
            let n = black_box(0);
        }
        v.iter().intersperse(&n)
    }
    intersperse_with {
        {
            let v = black_box(vec![0; 1024]);
            let n = black_box(0);
        }
        v.iter().intersperse_with(|| &n)
    }
    combinations1 {
        {
            let v = black_box(vec![0; 1792]);
        }
        v.iter().combinations(1)
    }
    combinations2 {
        {
            let v = black_box(vec![0; 60]);
        }
        v.iter().combinations(2)
    }
    combinations3 {
        {
            let v = black_box(vec![0; 23]);
        }
        v.iter().combinations(3)
    }
    combinations4 {
        {
            let v = black_box(vec![0; 16]);
        }
        v.iter().combinations(4)
    }
    combinations_with_replacement1 {
        {
            let v = black_box(vec![0; 4096]);
        }
        v.iter().combinations_with_replacement(1)
    }
    combinations_with_replacement2 {
        {
            let v = black_box(vec![0; 90]);
        }
        v.iter().combinations_with_replacement(2)
    }
    combinations_with_replacement3 {
        {
            let v = black_box(vec![0; 28]);
        }
        v.iter().combinations_with_replacement(3)
    }
    combinations_with_replacement4 {
        {
            let v = black_box(vec![0; 16]);
        }
        v.iter().combinations_with_replacement(4)
    }
    permutations1 {
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().permutations(1)
    }
    permutations2 {
        {
            let v = black_box(vec![0; 36]);
        }
        v.iter().permutations(2)
    }
    permutations3 {
        {
            let v = black_box(vec![0; 12]);
        }
        v.iter().permutations(3)
    }
    permutations4 {
        {
            let v = black_box(vec![0; 8]);
        }
        v.iter().permutations(4)
    }
    powerset {
        {
            let v = black_box(vec![0; 10]);
        }
        v.iter().powerset()
    }
    while_some {
        {}
        (0..)
            .map(black_box)
            .map(|i| char::from_digit(i, 16))
            .while_some()
    }
    with_position {
        ExactSizeIterator
        {
            let v = black_box((0..10240).collect_vec());
        }
        v.iter().with_position()
    }
    zip_longest {
        DoubleEndedIterator
        ExactSizeIterator
        {
            let xs = black_box(vec![0; 1024]);
            let ys = black_box(vec![0; 768]);
        }
        xs.iter().zip_longest(ys.iter())
    }
    zip_eq {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        v.iter().zip_eq(v.iter().rev())
    }
    multizip {
        DoubleEndedIterator
        ExactSizeIterator
        {
            let v1 = black_box(vec![0; 1024]);
            let v2 = black_box(vec![0; 768]);
            let v3 = black_box(vec![0; 2048]);
        }
        itertools::multizip((&v1, &v2, &v3))
    }
    izip {
        DoubleEndedIterator
        ExactSizeIterator
        {
            let v1 = black_box(vec![0; 1024]);
            let v2 = black_box(vec![0; 768]);
            let v3 = black_box(vec![0; 2048]);
        }
        itertools::izip!(&v1, &v2, &v3)
    }
    put_back {
        {
            let v = black_box(vec![0; 1024]);
        }
        itertools::put_back(&v).with_value(black_box(&0))
    }
    put_back_n {
        {
            let v1 = black_box(vec![0; 1024]);
            let v2 = black_box(vec![0; 16]);
        }
        {
            let mut it = itertools::put_back_n(&v1);
            for n in &v2 {
                it.put_back(n);
            }
            it
        }
    }
    exactly_one_error {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
        }
        // Use `at_most_one` would be similar.
        v.iter().exactly_one().unwrap_err()
    }
    multipeek {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
            let n = black_box(16);
        }
        {
            let mut it = v.iter().multipeek();
            for _ in 0..n {
                it.peek();
            }
            it
        }
    }
    peek_nth {
        ExactSizeIterator
        {
            let v = black_box(vec![0; 1024]);
            let n = black_box(16);
        }
        {
            let mut it = itertools::peek_nth(&v);
            it.peek_nth(n);
            it
        }
    }
    repeat_n {
        DoubleEndedIterator
        ExactSizeIterator
        {}
        itertools::repeat_n(black_box(0), black_box(1024))
    }
    merge {
        {
            let v1 = black_box((0..1024).collect_vec());
            let v2 = black_box((0..768).collect_vec());
        }
        v1.iter().merge(&v2)
    }
    merge_by {
        {
            let v1 = black_box((0..1024).collect_vec());
            let v2 = black_box((0..768).collect_vec());
        }
        v1.iter().merge_by(&v2, PartialOrd::ge)
    }
    merge_join_by_ordering {
        {
            let v1 = black_box((0..1024).collect_vec());
            let v2 = black_box((0..768).collect_vec());
        }
        v1.iter().merge_join_by(&v2, Ord::cmp)
    }
    merge_join_by_bool {
        {
            let v1 = black_box((0..1024).collect_vec());
            let v2 = black_box((0..768).collect_vec());
        }
        v1.iter().merge_join_by(&v2, PartialOrd::ge)
    }
    kmerge {
        {
            let vs = black_box(vec![vec![0; 1024], vec![0; 256], vec![0; 768]]);
        }
        vs.iter().kmerge()
    }
    kmerge_by {
        {
            let vs = black_box(vec![vec![0; 1024], vec![0; 256], vec![0; 768]]);
        }
        vs.iter().kmerge_by(PartialOrd::ge)
    }
    map_into {
        DoubleEndedIterator
        ExactSizeIterator
        {
            let v = black_box(vec![0_u8; 1024]);
        }
        v.iter().copied().map_into::<u32>()
    }
    map_ok {
        DoubleEndedIterator
        ExactSizeIterator
        {
            let v = black_box((0_u32..1024)
                .map(|x| if x % 2 == 1 { Err(x) } else { Ok(x) })
                .collect_vec());
        }
        v.iter().copied().map_ok(|x| x + 1)
    }
    filter_ok {
        {
            let v = black_box((0_u32..1024)
                .map(|x| if x % 2 == 1 { Err(x) } else { Ok(x) })
                .collect_vec());
        }
        v.iter().copied().filter_ok(|x| x % 3 == 0)
    }
    filter_map_ok {
        {
            let v = black_box((0_u32..1024)
                .map(|x| if x % 2 == 1 { Err(x) } else { Ok(x) })
                .collect_vec());
        }
        v.iter().copied().filter_map_ok(|x| if x % 3 == 0 { Some(x + 1) } else { None })
    }
    flatten_ok {
        DoubleEndedIterator
        {
            let d = black_box(vec![0; 8]);
            let v = black_box((0..512)
                .map(|x| if x % 2 == 0 { Ok(&d) } else { Err(x) })
                .collect_vec());
        }
        v.iter().copied().flatten_ok()
    }
}
