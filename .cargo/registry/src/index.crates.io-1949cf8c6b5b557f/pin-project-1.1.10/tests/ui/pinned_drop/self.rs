// SPDX-License-Identifier: Apache-2.0 OR MIT

pub mod self_in_macro_def {
    use std::pin::Pin;

    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    pub struct S {
        f: (),
    }

    #[pinned_drop]
    impl PinnedDrop for S {
        fn drop(self: Pin<&mut Self>) {
            macro_rules! t {
                () => {{
                    let _ = self; //~ ERROR E0434

                    fn f(self: ()) {} //~ ERROR `self` parameter is only allowed in associated functions
                }};
            }
            t!();
        }
    }
}

pub mod self_span {
    use std::pin::Pin;

    use pin_project::{pin_project, pinned_drop};

    #[pin_project(PinnedDrop)]
    pub struct S {
        f: (),
    }

    #[pinned_drop]
    impl PinnedDrop for S {
        fn drop(self: Pin<&mut Self>) {
            let _: () = self; //~ ERROR E0308
            let _: Self = Self; //~ ERROR E0423
        }
    }

    #[pin_project(PinnedDrop)]
    pub enum E {
        V { f: () },
    }

    #[pinned_drop]
    impl PinnedDrop for E {
        fn drop(self: Pin<&mut Self>) {
            let _: () = self; //~ ERROR E0308
            let _: Self = Self::V; //~ ERROR E0533
        }
    }
}

fn main() {}
