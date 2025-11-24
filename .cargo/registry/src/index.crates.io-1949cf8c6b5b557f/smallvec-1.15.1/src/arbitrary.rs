use crate::{Array, SmallVec};
use arbitrary::{Arbitrary, Unstructured};

impl<'a, A: Array> Arbitrary<'a> for SmallVec<A>
where
    <A as Array>::Item: Arbitrary<'a>,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        u.arbitrary_iter()?.collect()
    }

    fn arbitrary_take_rest(u: Unstructured<'a>) -> arbitrary::Result<Self> {
        u.arbitrary_take_rest_iter()?.collect()
    }

    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        arbitrary::size_hint::and(<usize as Arbitrary>::size_hint(depth), (0, None))
    }
}
