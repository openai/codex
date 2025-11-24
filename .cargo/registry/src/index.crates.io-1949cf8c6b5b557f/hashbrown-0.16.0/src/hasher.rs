#[cfg(feature = "default-hasher")]
use {
    core::hash::{BuildHasher, Hasher},
    foldhash::fast::RandomState,
};

/// Default hash builder for the `S` type parameter of
/// [`HashMap`](crate::HashMap) and [`HashSet`](crate::HashSet).
///
/// This only implements `BuildHasher` when the "default-hasher" crate feature
/// is enabled; otherwise it just serves as a placeholder, and a custom `S` type
/// must be used to have a fully functional `HashMap` or `HashSet`.
#[derive(Clone, Debug, Default)]
pub struct DefaultHashBuilder {
    #[cfg(feature = "default-hasher")]
    inner: RandomState,
}

#[cfg(feature = "default-hasher")]
impl BuildHasher for DefaultHashBuilder {
    type Hasher = DefaultHasher;

    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        DefaultHasher {
            inner: self.inner.build_hasher(),
        }
    }
}

/// Default hasher for [`HashMap`](crate::HashMap) and [`HashSet`](crate::HashSet).
#[cfg(feature = "default-hasher")]
#[derive(Clone)]
pub struct DefaultHasher {
    inner: <RandomState as BuildHasher>::Hasher,
}

#[cfg(feature = "default-hasher")]
macro_rules! forward_writes {
    ($( $write:ident ( $ty:ty ) , )*) => {$(
        #[inline(always)]
        fn $write(&mut self, arg: $ty) {
            self.inner.$write(arg);
        }
    )*}
}

#[cfg(feature = "default-hasher")]
impl Hasher for DefaultHasher {
    forward_writes! {
        write(&[u8]),
        write_u8(u8),
        write_u16(u16),
        write_u32(u32),
        write_u64(u64),
        write_u128(u128),
        write_usize(usize),
        write_i8(i8),
        write_i16(i16),
        write_i32(i32),
        write_i64(i64),
        write_i128(i128),
        write_isize(isize),
    }

    // feature(hasher_prefixfree_extras)
    #[cfg(feature = "nightly")]
    forward_writes! {
        write_length_prefix(usize),
        write_str(&str),
    }

    #[inline(always)]
    fn finish(&self) -> u64 {
        self.inner.finish()
    }
}
