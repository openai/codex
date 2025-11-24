//! This example shows how to wrap a data structure in a mutex to achieve safe mutability.
extern crate lazy_static;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref MUTEX_MAP: Mutex<HashMap<u32, &'static str>> = {
        let mut m = HashMap::new();
        m.insert(0, "foo");
        m.insert(1, "bar");
        m.insert(2, "baz");
        Mutex::new(m)
    };
}

fn main() {
    MUTEX_MAP.lock().unwrap().insert(0, "boo");
    println!(
        "The entry for `0` is \"{}\".",
        MUTEX_MAP.lock().unwrap().get(&0).unwrap()
    );
}
