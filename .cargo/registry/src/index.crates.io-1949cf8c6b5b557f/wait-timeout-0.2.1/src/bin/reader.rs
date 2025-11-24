use std::io::{stdin, Read};

#[allow(unused_must_use)]
fn main() {
    let mut buffer: [u8; 32] = Default::default();
    stdin().read(&mut buffer);
}
