use crate::page::{
    slot::{Generation, RefCount},
    Addr,
};
use crate::Pack;
use std::{fmt, marker::PhantomData};
/// Configuration parameters which can be overridden to tune the behavior of a slab.
pub trait Config: Sized {
    /// The maximum number of threads which can access the slab.
    ///
    /// This value (rounded to a power of two) determines the number of shards
    /// in the slab. If a thread is created, accesses the slab, and then terminates,
    /// its shard may be reused and thus does not count against the maximum
    /// number of threads once the thread has terminated.
    const MAX_THREADS: usize = DefaultConfig::MAX_THREADS;
    /// The maximum number of pages in each shard in the slab.
    ///
    /// This value, in combination with `INITIAL_PAGE_SIZE`, determines how many
    /// bits of each index are used to represent page addresses.
    const MAX_PAGES: usize = DefaultConfig::MAX_PAGES;
    /// The size of the first page in each shard.
    ///
    /// When a page in a shard has been filled with values, a new page
    /// will be allocated that is twice as large as the previous page. Thus, the
    /// second page will be twice this size, and the third will be four times
    /// this size, and so on.
    ///
    /// Note that page sizes must be powers of two. If this value is not a power
    /// of two, it will be rounded to the next power of two.
    const INITIAL_PAGE_SIZE: usize = DefaultConfig::INITIAL_PAGE_SIZE;
    /// Sets a number of high-order bits in each index which are reserved from
    /// user code.
    ///
    /// Note that these bits are taken from the generation counter; if the page
    /// address and thread IDs are configured to use a large number of bits,
    /// reserving additional bits will decrease the period of the generation
    /// counter. These should thus be used relatively sparingly, to ensure that
    /// generation counters are able to effectively prevent the ABA problem.
    const RESERVED_BITS: usize = 0;
}

pub(crate) trait CfgPrivate: Config {
    const USED_BITS: usize = Generation::<Self>::LEN + Generation::<Self>::SHIFT;
    const INITIAL_SZ: usize = next_pow2(Self::INITIAL_PAGE_SIZE);
    const MAX_SHARDS: usize = next_pow2(Self::MAX_THREADS - 1);
    const ADDR_INDEX_SHIFT: usize = Self::INITIAL_SZ.trailing_zeros() as usize + 1;

    fn page_size(n: usize) -> usize {
        Self::INITIAL_SZ * 2usize.pow(n as _)
    }

    fn debug() -> DebugConfig<Self> {
        DebugConfig { _cfg: PhantomData }
    }

    fn validate() {
        assert!(
            Self::INITIAL_SZ.is_power_of_two(),
            "invalid Config: {:#?}",
            Self::debug(),
        );
        assert!(
            Self::INITIAL_SZ <= Addr::<Self>::BITS,
            "invalid Config: {:#?}",
            Self::debug()
        );

        assert!(
            Generation::<Self>::BITS >= 3,
            "invalid Config: {:#?}\ngeneration counter should be at least 3 bits!",
            Self::debug()
        );

        assert!(
            Self::USED_BITS <= WIDTH,
            "invalid Config: {:#?}\ntotal number of bits per index is too large to fit in a word!",
            Self::debug()
        );

        assert!(
            WIDTH - Self::USED_BITS >= Self::RESERVED_BITS,
            "invalid Config: {:#?}\nindices are too large to fit reserved bits!",
            Self::debug()
        );

        assert!(
            RefCount::<Self>::MAX > 1,
            "invalid config: {:#?}\n maximum concurrent references would be {}",
            Self::debug(),
            RefCount::<Self>::MAX,
        );
    }

    #[inline(always)]
    fn unpack<A: Pack<Self>>(packed: usize) -> A {
        A::from_packed(packed)
    }

    #[inline(always)]
    fn unpack_addr(packed: usize) -> Addr<Self> {
        Self::unpack(packed)
    }

    #[inline(always)]
    fn unpack_tid(packed: usize) -> crate::Tid<Self> {
        Self::unpack(packed)
    }

    #[inline(always)]
    fn unpack_gen(packed: usize) -> Generation<Self> {
        Self::unpack(packed)
    }
}
impl<C: Config> CfgPrivate for C {}

/// Default slab configuration values.
#[derive(Copy, Clone)]
pub struct DefaultConfig {
    _p: (),
}

pub(crate) struct DebugConfig<C: Config> {
    _cfg: PhantomData<fn(C)>,
}

pub(crate) const WIDTH: usize = std::mem::size_of::<usize>() * 8;

pub(crate) const fn next_pow2(n: usize) -> usize {
    let pow2 = n.count_ones() == 1;
    let zeros = n.leading_zeros();
    1 << (WIDTH - zeros as usize - pow2 as usize)
}

// === impl DefaultConfig ===

impl Config for DefaultConfig {
    const INITIAL_PAGE_SIZE: usize = 32;

    #[cfg(target_pointer_width = "64")]
    const MAX_THREADS: usize = 4096;
    #[cfg(target_pointer_width = "32")]
    // TODO(eliza): can we find enough bits to give 32-bit platforms more threads?
    const MAX_THREADS: usize = 128;

    const MAX_PAGES: usize = WIDTH / 2;
}

impl fmt::Debug for DefaultConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Self::debug().fmt(f)
    }
}

impl<C: Config> fmt::Debug for DebugConfig<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(std::any::type_name::<C>())
            .field("initial_page_size", &C::INITIAL_SZ)
            .field("max_shards", &C::MAX_SHARDS)
            .field("max_pages", &C::MAX_PAGES)
            .field("used_bits", &C::USED_BITS)
            .field("reserved_bits", &C::RESERVED_BITS)
            .field("pointer_width", &WIDTH)
            .field("max_concurrent_references", &RefCount::<C>::MAX)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util;
    use crate::Slab;

    #[test]
    #[cfg_attr(loom, ignore)]
    #[should_panic]
    fn validates_max_refs() {
        struct GiantGenConfig;

        // Configure the slab with a very large number of bits for the generation
        // counter. This will only leave 1 bit to use for the slot reference
        // counter, which will fail to validate.
        impl Config for GiantGenConfig {
            const INITIAL_PAGE_SIZE: usize = 1;
            const MAX_THREADS: usize = 1;
            const MAX_PAGES: usize = 1;
        }

        let _slab = Slab::<usize>::new_with_config::<GiantGenConfig>();
    }

    #[test]
    #[cfg_attr(loom, ignore)]
    fn big() {
        let slab = Slab::new();

        for i in 0..10000 {
            println!("{:?}", i);
            let k = slab.insert(i).expect("insert");
            assert_eq!(slab.get(k).expect("get"), i);
        }
    }

    #[test]
    #[cfg_attr(loom, ignore)]
    fn custom_page_sz() {
        let slab = Slab::new_with_config::<test_util::TinyConfig>();

        for i in 0..4096 {
            println!("{}", i);
            let k = slab.insert(i).expect("insert");
            assert_eq!(slab.get(k).expect("get"), i);
        }
    }
}
