//! Specialized fuzzing for flags types using `arbitrary`.

use crate::Flags;

/**
Generate some arbitrary flags value with only known bits set.
*/
pub fn arbitrary<'a, B: Flags>(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<B>
where
    B::Bits: arbitrary::Arbitrary<'a>,
{
    B::from_bits(u.arbitrary()?).ok_or(arbitrary::Error::IncorrectFormat)
}

#[cfg(test)]
mod tests {
    use arbitrary::Arbitrary;

    bitflags! {
        #[derive(Arbitrary)]
        struct Color: u32 {
            const RED = 0x1;
            const GREEN = 0x2;
            const BLUE = 0x4;
        }
    }

    #[test]
    fn test_arbitrary() {
        let mut unstructured = arbitrary::Unstructured::new(&[0_u8; 256]);
        let _color = Color::arbitrary(&mut unstructured);
    }
}
