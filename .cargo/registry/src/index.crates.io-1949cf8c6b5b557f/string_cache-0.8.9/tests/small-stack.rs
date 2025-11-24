// Regression test for https://github.com/servo/html5ever/issues/393
//
// Create a dynamic atom − causing initialization of the global hash map −
// in a thread that has a small stack.
//
// This is a separate test program rather than a `#[test] fn` among others
// to make sure that nothing else has already initialized the map in this process.
fn main() {
    std::thread::Builder::new()
        .stack_size(50_000)
        .spawn(|| {
            let _atom = string_cache::DefaultAtom::from("12345678");
        })
        .unwrap()
        .join()
        .unwrap()
}
