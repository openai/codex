#![allow(dead_code)]

#[derive(ts_rs::TS)]
struct Test<T> {
    field: T,
}
