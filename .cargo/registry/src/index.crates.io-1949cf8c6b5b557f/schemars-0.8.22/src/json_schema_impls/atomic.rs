use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::sync::atomic::*;

forward_impl!(AtomicBool => bool);

forward_impl!(AtomicI8 => i8);
forward_impl!(AtomicI16 => i16);
forward_impl!(AtomicI32 => i32);
#[cfg(std_atomic64)]
forward_impl!(AtomicI64 => i64);
forward_impl!(AtomicIsize => isize);

forward_impl!(AtomicU8 => u8);
forward_impl!(AtomicU16 => u16);
forward_impl!(AtomicU32 => u32);
#[cfg(std_atomic64)]
forward_impl!(AtomicU64 => u64);
forward_impl!(AtomicUsize => usize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::schema_object_for;
    use pretty_assertions::assert_eq;

    #[test]
    fn schema_for_atomics() {
        let atomic_schema = schema_object_for::<(
            AtomicBool,
            AtomicI8,
            AtomicI16,
            AtomicI32,
            AtomicI64,
            AtomicIsize,
            AtomicU8,
            AtomicU16,
            AtomicU32,
            AtomicU64,
            AtomicUsize,
        )>();
        let basic_schema =
            schema_object_for::<(bool, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize)>();
        assert_eq!(atomic_schema, basic_schema);
    }
}
