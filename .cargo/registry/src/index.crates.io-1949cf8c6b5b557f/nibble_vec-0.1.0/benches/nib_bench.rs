use criterion::{criterion_group, criterion_main, Criterion};
use nibble_vec::Nibblet;

fn even_8to5() -> Nibblet {
    Nibblet::from_byte_vec(vec![8 << 4 | 7, 6 << 4 | 5])
}

fn odd_11to9() -> Nibblet {
    let mut result = Nibblet::from_byte_vec(vec![11 << 4 | 10]);
    result.push(9);
    result
}

fn split_test(nibble_vec: &Nibblet, idx: usize) {
    let mut init = nibble_vec.clone();
    let _tail = init.split(idx);
}

fn nib_split_even_bench(b: &mut Criterion) {
    let even_length = even_8to5();
    b.bench_function("nibvec split even", |b| {
        b.iter(|| {
            split_test(&even_length, 1);
            split_test(&even_length, 2);
        })
    });
}

fn nib_make_split_bench(b: &mut Criterion) {
    b.bench_function("nibvec split odd", |b| {
        let odd_length = odd_11to9();
        b.iter(|| {
            split_test(&odd_length, 0);
            split_test(&odd_length, 1);
        })
    });
}

fn nib_get_bench(b: &mut Criterion) {
    b.bench_function("nib get on vec of 9 elements", |b| {
        let v = vec![243, 2, 3, 251, 5, 6, 7, 8, 255];
        let nv = Nibblet::from(v.clone());
        b.iter(|| {
            for (i, _) in v.iter().enumerate() {
                nv.get(i);
            }
        })
    });
}

fn join_test(vec1: &Nibblet, vec2: &Nibblet) {
    let _joined = vec1.clone().join(vec2);
}

fn nib_join_bench(b: &mut Criterion) {
    b.bench_function("join even nibvec to odd nib", |b| {
        b.iter(|| {
            let v1 = even_8to5();
            let v2 = odd_11to9();
            join_test(&v1, &v2);
            join_test(&v1, &v1);
        });
    });
}

fn nib_from_into_bench(b: &mut Criterion) {
    b.bench_function("nib from vec and into vec", |b| {
        b.iter(|| {
            let x = vec![10, 11, 12, 13, 14, 15, 16];
            let nv = Nibblet::from_byte_vec(x);
            let v: Vec<u8> = nv.into();
            let _nv2 = Nibblet::from(v);
        });
    });
}

fn nib_cmp_bench(b: &mut Criterion) {
    b.bench_function("bench eq and not eq", |b| {
        let nv = Nibblet::from_byte_vec(vec![10, 11, 12, 13, 14, 15, 16]);
        let nv_eq = Nibblet::from_byte_vec(vec![10, 11, 12, 13, 14, 15, 16]);
        let nv_not_eq = Nibblet::from_byte_vec(vec![1, 1, 2, 3, 4, 5, 6]);
        b.iter(|| {
            let _a = nv == nv_eq;
            let _b = nv == nv_not_eq;
            let _c = nv_eq != nv_not_eq;
        });
    });
}

criterion_group!(
    benches,
    nib_split_even_bench,
    nib_make_split_bench,
    nib_join_bench,
    nib_get_bench,
    nib_from_into_bench,
    nib_cmp_bench
);
criterion_main!(benches);
