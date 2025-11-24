use criterion::{criterion_group, criterion_main, Criterion};
use itertools::Itertools;

fn s1(a: u32) -> u32 {
    a
}

fn s2(a: u32, b: u32) -> u32 {
    a + b
}

fn s3(a: u32, b: u32, c: u32) -> u32 {
    a + b + c
}

fn s4(a: u32, b: u32, c: u32, d: u32) -> u32 {
    a + b + c + d
}

fn sum_s1(s: &[u32]) -> u32 {
    s1(s[0])
}

fn sum_s2(s: &[u32]) -> u32 {
    s2(s[0], s[1])
}

fn sum_s3(s: &[u32]) -> u32 {
    s3(s[0], s[1], s[2])
}

fn sum_s4(s: &[u32]) -> u32 {
    s4(s[0], s[1], s[2], s[3])
}

fn sum_t1(s: &(&u32, )) -> u32 {
    s1(*s.0)
}

fn sum_t2(s: &(&u32, &u32)) -> u32 {
    s2(*s.0, *s.1)
}

fn sum_t3(s: &(&u32, &u32, &u32)) -> u32 {
    s3(*s.0, *s.1, *s.2)
}

fn sum_t4(s: &(&u32, &u32, &u32, &u32)) -> u32 {
    s4(*s.0, *s.1, *s.2, *s.3)
}

macro_rules! def_benchs {
    ($N:expr;
     $BENCH_GROUP:ident,
     $TUPLE_FUN:ident,
     $TUPLES:ident,
     $TUPLE_WINDOWS:ident;
     $SLICE_FUN:ident,
     $CHUNKS:ident,
     $WINDOWS:ident;
     $FOR_CHUNKS:ident,
     $FOR_WINDOWS:ident
     ) => (
        fn $FOR_CHUNKS(c: &mut Criterion) {
            let v: Vec<u32> = (0.. $N * 1_000).collect();
            let mut s = 0;
            c.bench_function(&stringify!($FOR_CHUNKS).replace('_', " "), move |b| {
                b.iter(|| {
                    let mut j = 0;
                    for _ in 0..1_000 {
                        s += $SLICE_FUN(&v[j..(j + $N)]);
                        j += $N;
                    }
                    s
                })
            });
        }

        fn $FOR_WINDOWS(c: &mut Criterion) {
            let v: Vec<u32> = (0..1_000).collect();
            let mut s = 0;
            c.bench_function(&stringify!($FOR_WINDOWS).replace('_', " "), move |b| {
                b.iter(|| {
                    for i in 0..(1_000 - $N) {
                        s += $SLICE_FUN(&v[i..(i + $N)]);
                    }
                    s
                })
            });
        }

        fn $TUPLES(c: &mut Criterion) {
            let v: Vec<u32> = (0.. $N * 1_000).collect();
            let mut s = 0;
            c.bench_function(&stringify!($TUPLES).replace('_', " "), move |b| {
                b.iter(|| {
                    for x in v.iter().tuples() {
                        s += $TUPLE_FUN(&x);
                    }
                    s
                })
            });
        }

        fn $CHUNKS(c: &mut Criterion) {
            let v: Vec<u32> = (0.. $N * 1_000).collect();
            let mut s = 0;
            c.bench_function(&stringify!($CHUNKS).replace('_', " "), move |b| {
                b.iter(|| {
                    for x in v.chunks($N) {
                        s += $SLICE_FUN(x);
                    }
                    s
                })
            });
        }

        fn $TUPLE_WINDOWS(c: &mut Criterion) {
            let v: Vec<u32> = (0..1_000).collect();
            let mut s = 0;
            c.bench_function(&stringify!($TUPLE_WINDOWS).replace('_', " "), move |b| {
                b.iter(|| {
                    for x in v.iter().tuple_windows() {
                        s += $TUPLE_FUN(&x);
                    }
                    s
                })
            });
        }

        fn $WINDOWS(c: &mut Criterion) {
            let v: Vec<u32> = (0..1_000).collect();
            let mut s = 0;
            c.bench_function(&stringify!($WINDOWS).replace('_', " "), move |b| {
                b.iter(|| {
                    for x in v.windows($N) {
                        s += $SLICE_FUN(x);
                    }
                    s
                })
            });
        }

        criterion_group!(
            $BENCH_GROUP,
            $FOR_CHUNKS,
            $FOR_WINDOWS,
            $TUPLES,
            $CHUNKS,
            $TUPLE_WINDOWS,
            $WINDOWS,
        );
    )
}

def_benchs!{
    1;
    benches_1,
    sum_t1,
    tuple_chunks_1,
    tuple_windows_1;
    sum_s1,
    slice_chunks_1,
    slice_windows_1;
    for_chunks_1,
    for_windows_1
}

def_benchs!{
    2;
    benches_2,
    sum_t2,
    tuple_chunks_2,
    tuple_windows_2;
    sum_s2,
    slice_chunks_2,
    slice_windows_2;
    for_chunks_2,
    for_windows_2
}

def_benchs!{
    3;
    benches_3,
    sum_t3,
    tuple_chunks_3,
    tuple_windows_3;
    sum_s3,
    slice_chunks_3,
    slice_windows_3;
    for_chunks_3,
    for_windows_3
}

def_benchs!{
    4;
    benches_4,
    sum_t4,
    tuple_chunks_4,
    tuple_windows_4;
    sum_s4,
    slice_chunks_4,
    slice_windows_4;
    for_chunks_4,
    for_windows_4
}

criterion_main!(
    benches_1,
    benches_2,
    benches_3,
    benches_4,
);
