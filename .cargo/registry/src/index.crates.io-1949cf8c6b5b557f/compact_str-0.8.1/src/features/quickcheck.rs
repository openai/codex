//! Implements the [`quickcheck::Arbitrary`] trait for [`CompactString`]

use alloc::boxed::Box;
use alloc::vec::Vec;

use quickcheck::{
    Arbitrary,
    Gen,
};

use crate::CompactString;

#[cfg_attr(docsrs, doc(cfg(feature = "quickcheck")))]
impl Arbitrary for CompactString {
    fn arbitrary(g: &mut Gen) -> CompactString {
        let max = g.size();

        // pick some value in [0, max]
        let x = usize::arbitrary(g);
        let ratio = (x as f64) / (usize::MAX as f64);
        let size = (ratio * max as f64) as usize;

        (0..size).map(|_| char::arbitrary(g)).collect()
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = CompactString>> {
        // Shrink a string by shrinking a vector of its characters.
        let chars: Vec<char> = self.chars().collect();
        Box::new(
            chars
                .shrink()
                .map(|x| x.into_iter().collect::<CompactString>()),
        )
    }
}

#[cfg(test)]
mod test {
    use alloc::string::String;

    use quickcheck_macros::quickcheck;

    use crate::CompactString;

    #[quickcheck]
    #[cfg_attr(miri, ignore)]
    fn quickcheck_sanity(compact: CompactString) {
        let control: String = compact.clone().into();
        assert_eq!(control, compact);
    }

    #[quickcheck]
    #[cfg_attr(miri, ignore)]
    fn quickcheck_inlines_strings(compact: CompactString) {
        if compact.len() <= core::mem::size_of::<String>() {
            assert!(!compact.is_heap_allocated())
        } else {
            assert!(compact.is_heap_allocated())
        }
    }
}
