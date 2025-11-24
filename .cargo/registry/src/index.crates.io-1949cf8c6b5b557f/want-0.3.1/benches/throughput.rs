#![feature(test)]

extern crate test;
extern crate want;

#[bench]
fn throughput(b: &mut test::Bencher) {
    let (mut gv, mut tk) = want::new();

    b.iter(move || {
        tk.want();
        assert!(gv.poll_want().unwrap().is_ready());
    });
}
