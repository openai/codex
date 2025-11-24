use std::path::Path;

use bencher::{benchmark_group, benchmark_main, Bencher};
use path_absolutize::Absolutize;

fn abs_no_dots(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("path/to/123/456");

    bencher.iter(|| path.absolutize());
}

fn abs_starts_with_a_single_dot(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("./path/to/123/456");

    bencher.iter(|| path.absolutize());
}

fn abs_starts_with_double_dots(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("../path/to/123/456");

    bencher.iter(|| path.absolutize());
}

fn abs_mix(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("./path/to/123/../456");

    bencher.iter(|| path.absolutize());
}

fn vabs_no_dots(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("path/to/123/456");
    let v_root = Path::new("/home");

    bencher.iter(|| path.absolutize_virtually(v_root));
}

fn vabs_starts_with_a_single_dot(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("./path/to/123/456");
    let v_root = Path::new("/home");

    bencher.iter(|| path.absolutize_virtually(v_root));
}

fn vabs_starts_with_double_dots(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("../path/to/123/456");
    let v_root = Path::new("/home");

    bencher.iter(|| path.absolutize_virtually(v_root));
}

fn vabs_mix(bencher: &mut Bencher) {
    #[cfg(feature = "unsafe_cache")]
    unsafe {
        path_absolutize::update_cwd()
    };

    let path = Path::new("./path/to/123/../456");
    let v_root = Path::new("/home");

    bencher.iter(|| path.absolutize_virtually(v_root));
}

benchmark_group!(
    absolutize,
    abs_no_dots,
    abs_starts_with_a_single_dot,
    abs_starts_with_double_dots,
    abs_mix
);
benchmark_group!(
    absolutize_virtually,
    vabs_no_dots,
    vabs_starts_with_a_single_dot,
    vabs_starts_with_double_dots,
    vabs_mix
);
benchmark_main!(absolutize, absolutize_virtually);
