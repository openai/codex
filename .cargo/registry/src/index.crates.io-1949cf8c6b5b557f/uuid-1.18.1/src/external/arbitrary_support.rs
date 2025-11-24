use crate::{
    non_nil::NonNilUuid,
    std::convert::{TryFrom, TryInto},
    Builder, Uuid,
};

use arbitrary::{Arbitrary, Unstructured};

impl Arbitrary<'_> for Uuid {
    fn arbitrary(u: &mut Unstructured<'_>) -> arbitrary::Result<Self> {
        let b = u
            .bytes(16)?
            .try_into()
            .map_err(|_| arbitrary::Error::NotEnoughData)?;

        Ok(Builder::from_random_bytes(b).into_uuid())
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (16, Some(16))
    }
}
impl arbitrary::Arbitrary<'_> for NonNilUuid {
    fn arbitrary(u: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        let uuid = Uuid::arbitrary(u)?;
        Self::try_from(uuid).map_err(|_| arbitrary::Error::IncorrectFormat)
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (16, Some(16))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Variant, Version};

    #[test]
    fn test_arbitrary() {
        let mut bytes = Unstructured::new(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let uuid = Uuid::arbitrary(&mut bytes).unwrap();

        assert_eq!(Some(Version::Random), uuid.get_version());
        assert_eq!(Variant::RFC4122, uuid.get_variant());
    }

    #[test]
    fn test_arbitrary_empty() {
        let mut bytes = Unstructured::new(&[]);

        // Ensure we don't panic when building an arbitrary `Uuid`
        let uuid = Uuid::arbitrary(&mut bytes);

        assert!(uuid.is_err());
    }

    #[test]
    fn test_arbitrary_non_nil() {
        let mut bytes = Unstructured::new(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        let non_nil_uuid = NonNilUuid::arbitrary(&mut bytes).unwrap();
        let uuid: Uuid = non_nil_uuid.into();

        assert_eq!(Some(Version::Random), uuid.get_version());
        assert_eq!(Variant::RFC4122, uuid.get_variant());
        assert!(!uuid.is_nil());
    }
}
