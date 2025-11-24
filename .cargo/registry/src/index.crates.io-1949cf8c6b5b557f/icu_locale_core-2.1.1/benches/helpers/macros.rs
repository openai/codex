// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[macro_export]
macro_rules! overview {
    ($c:expr, $struct:ident, $data_str:expr, $compare:expr) => {
        $c.bench_function("overview", |b| {
            b.iter(|| {
                let mut values = vec![];
                for s in $data_str {
                    let value: Result<$struct, _> = black_box(s).parse();
                    values.push(value.expect("Parsing failed"));
                }
                let _ = values
                    .iter()
                    .filter(|&v| v.normalizing_eq($compare))
                    .count();

                values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
            })
        });
    };
}

#[macro_export]
macro_rules! construct {
    ($c:expr, $struct:ident, $struct_name:expr, $data_str:expr) => {
        $c.bench_function($struct_name, |b| {
            b.iter(|| {
                for s in $data_str {
                    let _: Result<$struct, _> = black_box(s).parse();
                }
            })
        });
    };
}

#[macro_export]
macro_rules! to_string {
    ($c:expr, $struct:ident, $struct_name:expr, $data:expr) => {
        $c.bench_function($struct_name, |b| {
            b.iter(|| {
                for s in $data {
                    let _ = black_box(s).to_string();
                }
            })
        });
        $c.bench_function(std::concat!($struct_name, "/writeable"), |b| {
            use writeable::Writeable;
            b.iter(|| {
                for s in $data {
                    let _ = black_box(s).write_to_string();
                }
            })
        });
    };
}

#[macro_export]
macro_rules! compare_struct {
    ($c:expr, $struct:ident, $struct_name:expr, $data1:expr, $data2:expr) => {
        $c.bench_function(BenchmarkId::new("struct", $struct_name), |b| {
            b.iter(|| {
                for (lid1, lid2) in $data1.iter().zip($data2.iter()) {
                    let _ = black_box(lid1) == black_box(lid2);
                }
            })
        });
    };
}

#[macro_export]
macro_rules! compare_str {
    ($c:expr, $struct:ident, $struct_name:expr, $data1:expr, $data2:expr) => {
        $c.bench_function(BenchmarkId::new("str", $struct_name), |b| {
            b.iter(|| {
                for (lid, s) in $data1.iter().zip($data2.iter()) {
                    let _ = black_box(lid).normalizing_eq(&black_box(s));
                }
            })
        });
        $c.bench_function(BenchmarkId::new("strict_cmp", $struct_name), |b| {
            b.iter(|| {
                for (lid, s) in $data1.iter().zip($data2.iter()) {
                    let _ = black_box(lid).strict_cmp(&black_box(s).as_str().as_bytes());
                }
            })
        });
    };
}

#[macro_export]
macro_rules! canonicalize {
    ($c:expr, $struct:ident, $struct_name:expr, $data:expr) => {
        $c.bench_function($struct_name, |b| {
            b.iter(|| {
                for s in $data {
                    let _ = black_box(s).to_string();
                }
                for s in $data {
                    let _ = $struct::normalize(black_box(s));
                }
            })
        });
    };
}
