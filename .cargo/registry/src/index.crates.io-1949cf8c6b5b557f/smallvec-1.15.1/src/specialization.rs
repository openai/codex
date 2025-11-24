// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementations that require `default fn`.

use super::{Array, SmallVec, SpecFrom};

impl<'a, A: Array> SpecFrom<A, &'a [A::Item]> for SmallVec<A>
where
    A::Item: Clone,
{
    #[inline]
    default fn spec_from(slice: &'a [A::Item]) -> SmallVec<A> {
        slice.into_iter().cloned().collect()
    }
}
