//! Implements the [`arbitrary::Arbitrary`] trait for [`CompactString`]

use arbitrary::{
    Arbitrary,
    Result,
    Unstructured,
};

use crate::CompactString;

#[cfg_attr(docsrs, doc(cfg(feature = "arbitrary")))]
impl<'a> Arbitrary<'a> for CompactString {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        <&str as Arbitrary>::arbitrary(u).map(CompactString::new)
    }

    fn arbitrary_take_rest(u: Unstructured<'a>) -> Result<Self> {
        <&str as Arbitrary>::arbitrary_take_rest(u).map(CompactString::new)
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        <&str as Arbitrary>::size_hint(depth)
    }
}

#[cfg(test)]
mod test {
    use arbitrary::{
        Arbitrary,
        Unstructured,
    };

    use crate::CompactString;

    #[test]
    fn arbitrary_sanity() {
        let mut data = Unstructured::new(&[42; 50]);
        let compact = CompactString::arbitrary(&mut data).expect("generate a CompactString");

        // we don't really care what the content of the CompactString is, just that one's generated
        assert!(!compact.is_empty());
    }

    #[test]
    fn arbitrary_inlines_strings() {
        let mut data = Unstructured::new(&[42; 20]);
        let compact = CompactString::arbitrary(&mut data).expect("generate a CompactString");

        // running this manually, we generate the string "**"
        assert!(!compact.is_heap_allocated());
    }
}
