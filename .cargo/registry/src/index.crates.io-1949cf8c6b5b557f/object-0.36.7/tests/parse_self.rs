#![cfg(feature = "read")]
use object::{File, Object};
use std::{env, fs};

#[test]
fn parse_self() {
    let exe = env::current_exe().unwrap();
    let data = fs::read(exe).unwrap();
    let object = File::parse(&*data).unwrap();
    assert!(object.entry() != 0);
    assert!(object.sections().count() != 0);
}

#[cfg(feature = "std")]
#[test]
fn parse_self_cache() {
    use object::read::{ReadCache, ReadRef};
    let exe = env::current_exe().unwrap();
    let file = fs::File::open(exe).unwrap();
    let cache = ReadCache::new(file);
    let data = cache.range(0, cache.len().unwrap());
    let object = File::parse(data).unwrap();
    assert!(object.entry() != 0);
    assert!(object.sections().count() != 0);
}
