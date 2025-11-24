#[macro_use]
extern crate lazy_static;

mod outer {
    pub mod inner {
        lazy_static! {
            pub(in outer) static ref FOO: () = ();
        }
    }
}

fn main() {
    assert_eq!(*outer::inner::FOO, ());
}
