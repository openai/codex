#[macro_use(defer)]
extern crate scopeguard;

use scopeguard::guard;

fn f() {
    defer! {
        println!("Called at return or panic");
    }
    panic!();
}

use std::fs::File;
use std::io::Write;

fn g() {
    let f = File::create("newfile.txt").unwrap();
    let mut file = guard(f, |f| {
        // write file at return or panic
        let _ = f.sync_all();
    });
    // access the file through the scope guard itself
    file.write_all(b"test me\n").unwrap();
}

fn main() {
    f();
    g();
}
