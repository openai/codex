#[test]
fn cached_multithreaded() {
    (0..12)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..1000 {
                    supports_color::on_cached(supports_color::Stream::Stdout);
                }
            })
        })
        .for_each(|thread| thread.join().unwrap());
}
