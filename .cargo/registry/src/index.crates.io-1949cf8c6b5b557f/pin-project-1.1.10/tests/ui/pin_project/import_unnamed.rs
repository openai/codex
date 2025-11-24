// SPDX-License-Identifier: Apache-2.0 OR MIT

// Only named projected types can be imported.
// See visibility.rs for named projected types.

mod pub_ {
    use pin_project::pin_project;

    #[pin_project]
    pub struct Default(());

    #[pin_project(project_replace)]
    pub struct Replace(());
}
#[allow(unused_imports)]
pub mod use_ {
    #[rustfmt::skip]
    use crate::pub_::__DefaultProjection; //~ ERROR E0432
    #[rustfmt::skip]
    use crate::pub_::__DefaultProjectionRef; //~ ERROR E0432
    #[rustfmt::skip]
    use crate::pub_::__ReplaceProjection; //~ ERROR E0432
    #[rustfmt::skip]
    use crate::pub_::__ReplaceProjectionOwned; //~ ERROR E0432
    #[rustfmt::skip]
    use crate::pub_::__ReplaceProjectionRef; //~ ERROR E0432

    // Confirm that the visibility of the original type is not changed.
    pub use crate::pub_::{Default, Replace};
}

fn main() {}
