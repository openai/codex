//! Implements the [`proptest::arbitrary::Arbitrary`] trait for [`CompactString`]

use alloc::string::String;

use proptest::arbitrary::StrategyFor;
use proptest::prelude::*;
use proptest::strategy::MapInto;
use proptest::string::StringParam;

use crate::CompactString;

#[cfg_attr(docsrs, doc(cfg(feature = "proptest")))]
impl Arbitrary for CompactString {
    type Parameters = StringParam;
    type Strategy = MapInto<StrategyFor<String>, Self>;

    fn arbitrary_with(a: Self::Parameters) -> Self::Strategy {
        any_with::<String>(a).prop_map_into()
    }
}

#[cfg(test)]
mod test {
    use alloc::string::String;

    use proptest::prelude::*;

    use crate::CompactString;

    const MAX_SIZE: usize = core::mem::size_of::<String>();

    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn proptest_sanity(compact: CompactString) {
            let control: String = compact.clone().into();
            assert_eq!(control, compact);
        }

        /// We rely on [`proptest`]'s `String` strategy for generating a `CompactString`. When
        /// converting from a `String` into a `CompactString`, if it's short enough we should
        /// eagerly inline strings
        #[test]
        #[cfg_attr(miri, ignore)]
        fn proptest_does_not_inline_strings(compact: CompactString) {
            if compact.len() <= MAX_SIZE {
                assert!(!compact.is_heap_allocated());
            } else {
                assert!(compact.is_heap_allocated());
            }
        }
    }
}
