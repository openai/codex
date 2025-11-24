use tree_magic_mini as tree_magic;

#[macro_use]
extern crate bencher;
use bencher::Bencher;

///Image benchmarks
fn image_gif(b: &mut Bencher) {
    b.iter(|| tree_magic::match_u8("image/gif", include_bytes!("image/gif")));
}
fn image_png(b: &mut Bencher) {
    b.iter(|| tree_magic::match_u8("image/png", include_bytes!("image/png")));
}

/// Archive tests
fn application_zip(b: &mut Bencher) {
    b.iter(|| tree_magic::match_u8("application/zip", include_bytes!("application/zip")));
}

/// Text tests
fn text_plain(b: &mut Bencher) {
    b.iter(|| tree_magic::match_u8("text/plain", include_bytes!("text/plain")));
}

benchmark_group!(benches, image_gif, image_png, application_zip, text_plain);
benchmark_main!(benches);
