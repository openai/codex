#![allow(dead_code)]

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "lifetimes/")]
struct S<'a> {
    s: &'a str,
}

#[derive(TS)]
#[ts(export, export_to = "lifetimes/")]
struct B<'a, T: 'a> {
    a: &'a T,
}

#[derive(TS)]
#[ts(export, export_to = "lifetimes/")]
struct A<'a> {
    a: &'a &'a &'a Vec<u32>,                        //Multiple References
    b: &'a Vec<B<'a, u32>>,                         //Nesting
    c: &'a std::collections::HashMap<String, bool>, //Multiple type args
}

#[test]
fn contains_borrow() {
    assert_eq!(S::decl(), "type S = { s: string, };")
}

#[test]
fn contains_borrow_type_args() {
    assert_eq!(
        A::decl(),
        "type A = { a: Array<number>, b: Array<B<number>>, c: { [key in string]?: boolean }, };"
    );
}
