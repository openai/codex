#![feature(test)]
#![allow(non_snake_case)]
#![allow(clippy::cast_lossless)]

extern crate test;

macro_rules! benches {
    ($($name:ident($value:expr))*) => {
        mod bench_itoa_format {
            use test::{Bencher, black_box};

            $(
                #[bench]
                fn $name(b: &mut Bencher) {
                    let mut buffer = itoa::Buffer::new();

                    b.iter(|| {
                        let printed = buffer.format(black_box($value));
                        black_box(printed);
                    });
                }
            )*
        }

        mod bench_std_fmt {
            use std::io::Write;
            use test::{Bencher, black_box};

            $(
                #[bench]
                fn $name(b: &mut Bencher) {
                    let mut buf = Vec::with_capacity(40);

                    b.iter(|| {
                        buf.clear();
                        write!(&mut buf, "{}", black_box($value)).unwrap();
                        black_box(&buf);
                    });
                }
            )*
        }
    }
}

benches! {
    bench_u64_0(0u64)
    bench_u64_half(u32::max_value() as u64)
    bench_u64_max(u64::max_value())

    bench_i16_0(0i16)
    bench_i16_min(i16::min_value())

    bench_u128_0(0u128)
    bench_u128_max(u128::max_value())
}
