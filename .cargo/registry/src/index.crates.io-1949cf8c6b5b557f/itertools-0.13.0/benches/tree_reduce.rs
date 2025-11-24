#![allow(deprecated)]

use criterion::{criterion_group, criterion_main, Criterion};
use itertools::{cloned, Itertools};

trait IterEx: Iterator {
    // Another efficient implementation against which to compare,
    // but needs `std` so is less desirable.
    fn tree_reduce_vec<F>(self, mut f: F) -> Option<Self::Item>
    where
        F: FnMut(Self::Item, Self::Item) -> Self::Item,
        Self: Sized,
    {
        let hint = self.size_hint().0;
        let cap = std::mem::size_of::<usize>() * 8 - hint.leading_zeros() as usize;
        let mut stack = Vec::with_capacity(cap);
        self.enumerate().for_each(|(mut i, mut x)| {
            while (i & 1) != 0 {
                x = f(stack.pop().unwrap(), x);
                i >>= 1;
            }
            stack.push(x);
        });
        stack.into_iter().fold1(f)
    }
}
impl<T: Iterator> IterEx for T {}

macro_rules! def_benchs {
    ($N:expr,
     $FUN:ident,
     $BENCH_NAME:ident,
     ) => {
        mod $BENCH_NAME {
            use super::*;

            pub fn sum(c: &mut Criterion) {
                let v: Vec<u32> = (0..$N).collect();

                c.bench_function(
                    &(stringify!($BENCH_NAME).replace('_', " ") + " sum"),
                    move |b| b.iter(|| cloned(&v).$FUN(|x, y| x + y)),
                );
            }

            pub fn complex_iter(c: &mut Criterion) {
                let u = (3..).take($N / 2);
                let v = (5..).take($N / 2);
                let it = u.chain(v);

                c.bench_function(
                    &(stringify!($BENCH_NAME).replace('_', " ") + " complex iter"),
                    move |b| b.iter(|| it.clone().map(|x| x as f32).$FUN(f32::atan2)),
                );
            }

            pub fn string_format(c: &mut Criterion) {
                // This goes quadratic with linear `fold1`, so use a smaller
                // size to not waste too much time in travis.  The allocations
                // in here are so expensive anyway that it'll still take
                // way longer per iteration than the other two benchmarks.
                let v: Vec<u32> = (0..($N / 4)).collect();

                c.bench_function(
                    &(stringify!($BENCH_NAME).replace('_', " ") + " string format"),
                    move |b| {
                        b.iter(|| {
                            cloned(&v)
                                .map(|x| x.to_string())
                                .$FUN(|x, y| format!("{} + {}", x, y))
                        })
                    },
                );
            }
        }

        criterion_group!(
            $BENCH_NAME,
            $BENCH_NAME::sum,
            $BENCH_NAME::complex_iter,
            $BENCH_NAME::string_format,
        );
    };
}

def_benchs! {
    10_000,
    fold1,
    fold1_10k,
}

def_benchs! {
    10_000,
    tree_reduce,
    tree_reduce_stack_10k,
}

def_benchs! {
    10_000,
    tree_reduce_vec,
    tree_reduce_vec_10k,
}

def_benchs! {
    100,
    fold1,
    fold1_100,
}

def_benchs! {
    100,
    tree_reduce,
    tree_reduce_stack_100,
}

def_benchs! {
    100,
    tree_reduce_vec,
    tree_reduce_vec_100,
}

def_benchs! {
    8,
    fold1,
    fold1_08,
}

def_benchs! {
    8,
    tree_reduce,
    tree_reduce_stack_08,
}

def_benchs! {
    8,
    tree_reduce_vec,
    tree_reduce_vec_08,
}

criterion_main!(
    fold1_10k,
    tree_reduce_stack_10k,
    tree_reduce_vec_10k,
    fold1_100,
    tree_reduce_stack_100,
    tree_reduce_vec_100,
    fold1_08,
    tree_reduce_stack_08,
    tree_reduce_vec_08,
);
