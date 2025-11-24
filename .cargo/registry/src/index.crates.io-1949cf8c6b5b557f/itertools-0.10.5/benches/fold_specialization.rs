use criterion::{criterion_group, criterion_main, Criterion};
use itertools::Itertools;

struct Unspecialized<I>(I);

impl<I> Iterator for Unspecialized<I>
where I: Iterator
{
    type Item = I::Item;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

mod specialization {
    use super::*;

    pub mod intersperse {
        use super::*;

        pub fn external(c: &mut Criterion)
        {
            let arr = [1; 1024];

            c.bench_function("external", move |b| {
                b.iter(|| {
                    let mut sum = 0;
                    for &x in arr.iter().intersperse(&0) {
                        sum += x;
                    }
                    sum
                })
            });
        }

        pub fn internal_specialized(c: &mut Criterion)
        {
            let arr = [1; 1024];

            c.bench_function("internal specialized", move |b| {
                b.iter(|| {
                    arr.iter().intersperse(&0).fold(0, |acc, x| acc + x)
                })
            });
        }

        pub fn internal_unspecialized(c: &mut Criterion)
        {
            let arr = [1; 1024];

            c.bench_function("internal unspecialized", move |b| {
                b.iter(|| {
                    Unspecialized(arr.iter().intersperse(&0)).fold(0, |acc, x| acc + x)
                })
            });
        }
    }
}

criterion_group!(
    benches,
    specialization::intersperse::external,
    specialization::intersperse::internal_specialized,
    specialization::intersperse::internal_unspecialized,
);
criterion_main!(benches);
