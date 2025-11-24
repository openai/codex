use tree_magic_mini as tree_magic;

#[macro_use]
extern crate bencher;
use bencher::Bencher;

///Image tests
fn image_gif(b: &mut Bencher) {
    b.iter(|| tree_magic::from_u8(include_bytes!("image/gif")));
}
fn image_png(b: &mut Bencher) {
    b.iter(|| tree_magic::from_u8(include_bytes!("image/png")));
}

/// Archive tests
fn application_zip(b: &mut Bencher) {
    b.iter(|| tree_magic::from_u8(include_bytes!("application/zip")));
}

/// Text tests
fn text_plain(b: &mut Bencher) {
    b.iter(|| tree_magic::from_u8(include_bytes!("text/plain")));
}

benchmark_group!(benches, image_gif, image_png, application_zip, text_plain);
benchmark_main!(benches);
