#![allow(unused)]

use ::ts_rs::TS;

mod ts_rs {}

#[derive(TS)]
struct Foo {
    x: u32,
}
