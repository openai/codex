use core::sync::atomic::*;

#[cfg(target_has_atomic = "8")]
forward_impl!(AtomicBool => bool);

#[cfg(target_has_atomic = "8")]
forward_impl!(AtomicI8 => i8);

#[cfg(target_has_atomic = "16")]
forward_impl!(AtomicI16 => i16);

#[cfg(target_has_atomic = "32")]
forward_impl!(AtomicI32 => i32);

#[cfg(target_has_atomic = "64")]
forward_impl!(AtomicI64 => i64);

#[cfg(target_has_atomic = "ptr")]
forward_impl!(AtomicIsize => isize);

#[cfg(target_has_atomic = "8")]
forward_impl!(AtomicU8 => u8);

#[cfg(target_has_atomic = "16")]
forward_impl!(AtomicU16 => u16);

#[cfg(target_has_atomic = "32")]
forward_impl!(AtomicU32 => u32);

#[cfg(target_has_atomic = "64")]
forward_impl!(AtomicU64 => u64);

#[cfg(target_has_atomic = "ptr")]
forward_impl!(AtomicUsize => usize);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_for;
    use pretty_assertions::assert_eq;

    #[test]
    fn schema_for_atomics() {
        let atomic_schema = schema_for!((
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
        ));
        let basic_schema = schema_for!((bool, i8, i16, i32, i64, isize, u8, u16, u32, u64, usize));
        assert_eq!(atomic_schema, basic_schema);
    }
}
