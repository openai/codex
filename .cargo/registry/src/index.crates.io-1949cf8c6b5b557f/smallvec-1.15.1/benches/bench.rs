#![feature(test)]
#![allow(deprecated)]

extern crate test;

use self::test::Bencher;
use smallvec::{ExtendFromSlice, smallvec, SmallVec};

const VEC_SIZE: usize = 16;
const SPILLED_SIZE: usize = 100;

trait Vector<T>: for<'a> From<&'a [T]> + Extend<T> + ExtendFromSlice<T> {
    fn new() -> Self;
    fn push(&mut self, val: T);
    fn pop(&mut self) -> Option<T>;
    fn remove(&mut self, p: usize) -> T;
    fn insert(&mut self, n: usize, val: T);
    fn from_elem(val: T, n: usize) -> Self;
    fn from_elems(val: &[T]) -> Self;
}

impl<T: Copy> Vector<T> for Vec<T> {
    fn new() -> Self {
        Self::with_capacity(VEC_SIZE)
    }

    fn push(&mut self, val: T) {
        self.push(val)
    }

    fn pop(&mut self) -> Option<T> {
        self.pop()
    }

    fn remove(&mut self, p: usize) -> T {
        self.remove(p)
    }

    fn insert(&mut self, n: usize, val: T) {
        self.insert(n, val)
    }

    fn from_elem(val: T, n: usize) -> Self {
        vec![val; n]
    }

    fn from_elems(val: &[T]) -> Self {
        val.to_owned()
    }
}

impl<T: Copy> Vector<T> for SmallVec<[T; VEC_SIZE]> {
    fn new() -> Self {
        Self::new()
    }

    fn push(&mut self, val: T) {
        self.push(val)
    }

    fn pop(&mut self) -> Option<T> {
        self.pop()
    }

    fn remove(&mut self, p: usize) -> T {
        self.remove(p)
    }

    fn insert(&mut self, n: usize, val: T) {
        self.insert(n, val)
    }

    fn from_elem(val: T, n: usize) -> Self {
        smallvec![val; n]
    }

    fn from_elems(val: &[T]) -> Self {
        SmallVec::from_slice(val)
    }
}

macro_rules! make_benches {
    ($typ:ty { $($b_name:ident => $g_name:ident($($args:expr),*),)* }) => {
        $(
            #[bench]
            fn $b_name(b: &mut Bencher) {
                $g_name::<$typ>($($args,)* b)
            }
        )*
    }
}

make_benches! {
    SmallVec<[u64; VEC_SIZE]> {
        bench_push => gen_push(SPILLED_SIZE as _),
        bench_push_small => gen_push(VEC_SIZE as _),
        bench_insert_push => gen_insert_push(SPILLED_SIZE as _),
        bench_insert_push_small => gen_insert_push(VEC_SIZE as _),
        bench_insert => gen_insert(SPILLED_SIZE as _),
        bench_insert_small => gen_insert(VEC_SIZE as _),
        bench_remove => gen_remove(SPILLED_SIZE as _),
        bench_remove_small => gen_remove(VEC_SIZE as _),
        bench_extend => gen_extend(SPILLED_SIZE as _),
        bench_extend_small => gen_extend(VEC_SIZE as _),
        bench_from_iter => gen_from_iter(SPILLED_SIZE as _),
        bench_from_iter_small => gen_from_iter(VEC_SIZE as _),
        bench_from_slice => gen_from_slice(SPILLED_SIZE as _),
        bench_from_slice_small => gen_from_slice(VEC_SIZE as _),
        bench_extend_from_slice => gen_extend_from_slice(SPILLED_SIZE as _),
        bench_extend_from_slice_small => gen_extend_from_slice(VEC_SIZE as _),
        bench_macro_from_elem => gen_from_elem(SPILLED_SIZE as _),
        bench_macro_from_elem_small => gen_from_elem(VEC_SIZE as _),
        bench_pushpop => gen_pushpop(),
    }
}

make_benches! {
    Vec<u64> {
        bench_push_vec => gen_push(SPILLED_SIZE as _),
        bench_push_vec_small => gen_push(VEC_SIZE as _),
        bench_insert_push_vec => gen_insert_push(SPILLED_SIZE as _),
        bench_insert_push_vec_small => gen_insert_push(VEC_SIZE as _),
        bench_insert_vec => gen_insert(SPILLED_SIZE as _),
        bench_insert_vec_small => gen_insert(VEC_SIZE as _),
        bench_remove_vec => gen_remove(SPILLED_SIZE as _),
        bench_remove_vec_small => gen_remove(VEC_SIZE as _),
        bench_extend_vec => gen_extend(SPILLED_SIZE as _),
        bench_extend_vec_small => gen_extend(VEC_SIZE as _),
        bench_from_iter_vec => gen_from_iter(SPILLED_SIZE as _),
        bench_from_iter_vec_small => gen_from_iter(VEC_SIZE as _),
        bench_from_slice_vec => gen_from_slice(SPILLED_SIZE as _),
        bench_from_slice_vec_small => gen_from_slice(VEC_SIZE as _),
        bench_extend_from_slice_vec => gen_extend_from_slice(SPILLED_SIZE as _),
        bench_extend_from_slice_vec_small => gen_extend_from_slice(VEC_SIZE as _),
        bench_macro_from_elem_vec => gen_from_elem(SPILLED_SIZE as _),
        bench_macro_from_elem_vec_small => gen_from_elem(VEC_SIZE as _),
        bench_pushpop_vec => gen_pushpop(),
    }
}

fn gen_push<V: Vector<u64>>(n: u64, b: &mut Bencher) {
    #[inline(never)]
    fn push_noinline<V: Vector<u64>>(vec: &mut V, x: u64) {
        vec.push(x);
    }

    b.iter(|| {
        let mut vec = V::new();
        for x in 0..n {
            push_noinline(&mut vec, x);
        }
        vec
    });
}

fn gen_insert_push<V: Vector<u64>>(n: u64, b: &mut Bencher) {
    #[inline(never)]
    fn insert_push_noinline<V: Vector<u64>>(vec: &mut V, x: u64) {
        vec.insert(x as usize, x);
    }

    b.iter(|| {
        let mut vec = V::new();
        for x in 0..n {
            insert_push_noinline(&mut vec, x);
        }
        vec
    });
}

fn gen_insert<V: Vector<u64>>(n: u64, b: &mut Bencher) {
    #[inline(never)]
    fn insert_noinline<V: Vector<u64>>(vec: &mut V, p: usize, x: u64) {
        vec.insert(p, x)
    }

    b.iter(|| {
        let mut vec = V::new();
        // Always insert at position 0 so that we are subject to shifts of
        // many different lengths.
        vec.push(0);
        for x in 0..n {
            insert_noinline(&mut vec, 0, x);
        }
        vec
    });
}

fn gen_remove<V: Vector<u64>>(n: usize, b: &mut Bencher) {
    #[inline(never)]
    fn remove_noinline<V: Vector<u64>>(vec: &mut V, p: usize) -> u64 {
        vec.remove(p)
    }

    b.iter(|| {
        let mut vec = V::from_elem(0, n as _);

        for _ in 0..n {
            remove_noinline(&mut vec, 0);
        }
    });
}

fn gen_extend<V: Vector<u64>>(n: u64, b: &mut Bencher) {
    b.iter(|| {
        let mut vec = V::new();
        vec.extend(0..n);
        vec
    });
}

fn gen_from_iter<V: Vector<u64>>(n: u64, b: &mut Bencher) {
    let v: Vec<u64> = (0..n).collect();
    b.iter(|| {
        let vec = V::from(&v);
        vec
    });
}

fn gen_from_slice<V: Vector<u64>>(n: u64, b: &mut Bencher) {
    let v: Vec<u64> = (0..n).collect();
    b.iter(|| {
        let vec = V::from_elems(&v);
        vec
    });
}

fn gen_extend_from_slice<V: Vector<u64>>(n: u64, b: &mut Bencher) {
    let v: Vec<u64> = (0..n).collect();
    b.iter(|| {
        let mut vec = V::new();
        vec.extend_from_slice(&v);
        vec
    });
}

fn gen_pushpop<V: Vector<u64>>(b: &mut Bencher) {
    #[inline(never)]
    fn pushpop_noinline<V: Vector<u64>>(vec: &mut V, x: u64) -> Option<u64> {
        vec.push(x);
        vec.pop()
    }

    b.iter(|| {
        let mut vec = V::new();
        for x in 0..SPILLED_SIZE as _ {
            pushpop_noinline(&mut vec, x);
        }
        vec
    });
}

fn gen_from_elem<V: Vector<u64>>(n: usize, b: &mut Bencher) {
    b.iter(|| {
        let vec = V::from_elem(42, n);
        vec
    });
}

#[bench]
fn bench_insert_many(b: &mut Bencher) {
    #[inline(never)]
    fn insert_many_noinline<I: IntoIterator<Item = u64>>(
        vec: &mut SmallVec<[u64; VEC_SIZE]>,
        index: usize,
        iterable: I,
    ) {
        vec.insert_many(index, iterable)
    }

    b.iter(|| {
        let mut vec = SmallVec::<[u64; VEC_SIZE]>::new();
        insert_many_noinline(&mut vec, 0, 0..SPILLED_SIZE as _);
        insert_many_noinline(&mut vec, 0, 0..SPILLED_SIZE as _);
        vec
    });
}

#[bench]
fn bench_insert_from_slice(b: &mut Bencher) {
    let v: Vec<u64> = (0..SPILLED_SIZE as _).collect();
    b.iter(|| {
        let mut vec = SmallVec::<[u64; VEC_SIZE]>::new();
        vec.insert_from_slice(0, &v);
        vec.insert_from_slice(0, &v);
        vec
    });
}

#[bench]
fn bench_macro_from_list(b: &mut Bencher) {
    b.iter(|| {
        let vec: SmallVec<[u64; 16]> = smallvec![
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 20, 24, 32, 36, 0x40, 0x80,
            0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x4000, 0x8000, 0x10000, 0x20000, 0x40000,
            0x80000, 0x100000,
        ];
        vec
    });
}

#[bench]
fn bench_macro_from_list_vec(b: &mut Bencher) {
    b.iter(|| {
        let vec: Vec<u64> = vec![
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 20, 24, 32, 36, 0x40, 0x80,
            0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x4000, 0x8000, 0x10000, 0x20000, 0x40000,
            0x80000, 0x100000,
        ];
        vec
    });
}
