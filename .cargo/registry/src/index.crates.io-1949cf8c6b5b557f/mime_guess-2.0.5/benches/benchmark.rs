#[macro_use]
extern crate criterion;
extern crate mime_guess;

use self::criterion::Criterion;

use mime_guess::from_ext;

include!("../src/mime_types.rs");

/// WARNING: this may take a while!
fn bench_mime_str(c: &mut Criterion) {
    c.bench_function("from_ext", |b| {
        for (mime_ext, _) in MIME_TYPES {
            b.iter(|| from_ext(mime_ext).first_raw());
        }
    });
}

fn bench_mime_str_uppercase(c: &mut Criterion) {
    c.bench_function("from_ext uppercased", |b| {
        let uppercased = MIME_TYPES.into_iter().map(|(s, _)| s.to_uppercase());

        for mime_ext in uppercased {
            b.iter(|| from_ext(&mime_ext).first_raw());
        }
    });
}

criterion_group!(benches, bench_mime_str, bench_mime_str_uppercase);
criterion_main!(benches);
