//! An implementation of the [Fowler–Noll–Vo hash function][chongo].
//!
//! ## About
//!
//! The FNV hash function is a custom `Hasher` implementation that is more
//! efficient for smaller hash keys.
//!
//! [The Rust FAQ states that][faq] while the default `Hasher` implementation,
//! SipHash, is good in many cases, it is notably slower than other algorithms
//! with short keys, such as when you have a map of integers to other values.
//! In cases like these, [FNV is demonstrably faster][graphs].
//!
//! Its disadvantages are that it performs badly on larger inputs, and
//! provides no protection against collision attacks, where a malicious user
//! can craft specific keys designed to slow a hasher down. Thus, it is
//! important to profile your program to ensure that you are using small hash
//! keys, and be certain that your program could not be exposed to malicious
//! inputs (including being a networked server).
//!
//! The Rust compiler itself uses FNV, as it is not worried about
//! denial-of-service attacks, and can assume that its inputs are going to be
//! small—a perfect use case for FNV.
//!
#![cfg_attr(feature = "std", doc = r#"

## Using FNV in a `HashMap`

The `FnvHashMap` type alias is the easiest way to use the standard library’s
`HashMap` with FNV.

```rust
use fnv::FnvHashMap;

let mut map = FnvHashMap::default();
map.insert(1, "one");
map.insert(2, "two");

map = FnvHashMap::with_capacity_and_hasher(10, Default::default());
map.insert(1, "one");
map.insert(2, "two");
```

Note, the standard library’s `HashMap::new` and `HashMap::with_capacity`
are only implemented for the `RandomState` hasher, so using `Default` to
get the hasher is the next best option.

## Using FNV in a `HashSet`

Similarly, `FnvHashSet` is a type alias for the standard library’s `HashSet`
with FNV.

```rust
use fnv::FnvHashSet;

let mut set = FnvHashSet::default();
set.insert(1);
set.insert(2);

set = FnvHashSet::with_capacity_and_hasher(10, Default::default());
set.insert(1);
set.insert(2);
```
"#)]
//!
//! [chongo]: http://www.isthe.com/chongo/tech/comp/fnv/index.html
//! [faq]: https://www.rust-lang.org/en-US/faq.html#why-are-rusts-hashmaps-slow
//! [graphs]: https://cglab.ca/~abeinges/blah/hash-rs/

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(all(not(feature = "std"), test))]
extern crate alloc;

#[cfg(feature = "std")]
use std::default::Default;
#[cfg(feature = "std")]
use std::hash::{Hasher, BuildHasherDefault};
#[cfg(feature = "std")]
use std::collections::{HashMap, HashSet};
#[cfg(not(feature = "std"))]
use core::default::Default;
#[cfg(not(feature = "std"))]
use core::hash::{Hasher, BuildHasherDefault};

/// An implementation of the Fowler–Noll–Vo hash function.
///
/// See the [crate documentation](index.html) for more details.
#[allow(missing_copy_implementations)]
pub struct FnvHasher(u64);

impl Default for FnvHasher {

    #[inline]
    fn default() -> FnvHasher {
        FnvHasher(0xcbf29ce484222325)
    }
}

impl FnvHasher {
    /// Create an FNV hasher starting with a state corresponding
    /// to the hash `key`.
    #[inline]
    pub fn with_key(key: u64) -> FnvHasher {
        FnvHasher(key)
    }
}

impl Hasher for FnvHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let FnvHasher(mut hash) = *self;

        for byte in bytes.iter() {
            hash = hash ^ (*byte as u64);
            hash = hash.wrapping_mul(0x100000001b3);
        }

        *self = FnvHasher(hash);
    }
}

/// A builder for default FNV hashers.
pub type FnvBuildHasher = BuildHasherDefault<FnvHasher>;

/// A `HashMap` using a default FNV hasher.
#[cfg(feature = "std")]
pub type FnvHashMap<K, V> = HashMap<K, V, FnvBuildHasher>;

/// A `HashSet` using a default FNV hasher.
#[cfg(feature = "std")]
pub type FnvHashSet<T> = HashSet<T, FnvBuildHasher>;


#[cfg(test)]
mod test {
    use super::*;
    #[cfg(feature = "std")]
    use std::hash::Hasher;
    #[cfg(not(feature = "std"))]
    use alloc::vec::Vec;

    fn fnv1a(bytes: &[u8]) -> u64 {
        let mut hasher = FnvHasher::default();
        hasher.write(bytes);
        hasher.finish()
    }

    fn repeat_10(bytes: &[u8]) -> Vec<u8> {
        (0..10).flat_map(|_| bytes.iter().cloned()).collect()
    }

    fn repeat_500(bytes: &[u8]) -> Vec<u8> {
        (0..500).flat_map(|_| bytes.iter().cloned()).collect()
    }

    #[test]
    fn basic_tests() {
        assert_eq!(fnv1a(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a(b"b"), 0xaf63df4c8601f1a5);
        assert_eq!(fnv1a(b"c"), 0xaf63de4c8601eff2);
        assert_eq!(fnv1a(b"d"), 0xaf63d94c8601e773);
        assert_eq!(fnv1a(b"e"), 0xaf63d84c8601e5c0);
        assert_eq!(fnv1a(b"f"), 0xaf63db4c8601ead9);
        assert_eq!(fnv1a(b"fo"), 0x08985907b541d342);
        assert_eq!(fnv1a(b"foo"), 0xdcb27518fed9d577);
        assert_eq!(fnv1a(b"foob"), 0xdd120e790c2512af);
        assert_eq!(fnv1a(b"fooba"), 0xcac165afa2fef40a);
        assert_eq!(fnv1a(b"foobar"), 0x85944171f73967e8);
        assert_eq!(fnv1a(b"\0"), 0xaf63bd4c8601b7df);
        assert_eq!(fnv1a(b"a\0"), 0x089be207b544f1e4);
        assert_eq!(fnv1a(b"b\0"), 0x08a61407b54d9b5f);
        assert_eq!(fnv1a(b"c\0"), 0x08a2ae07b54ab836);
        assert_eq!(fnv1a(b"d\0"), 0x0891b007b53c4869);
        assert_eq!(fnv1a(b"e\0"), 0x088e4a07b5396540);
        assert_eq!(fnv1a(b"f\0"), 0x08987c07b5420ebb);
        assert_eq!(fnv1a(b"fo\0"), 0xdcb28a18fed9f926);
        assert_eq!(fnv1a(b"foo\0"), 0xdd1270790c25b935);
        assert_eq!(fnv1a(b"foob\0"), 0xcac146afa2febf5d);
        assert_eq!(fnv1a(b"fooba\0"), 0x8593d371f738acfe);
        assert_eq!(fnv1a(b"foobar\0"), 0x34531ca7168b8f38);
        assert_eq!(fnv1a(b"ch"), 0x08a25607b54a22ae);
        assert_eq!(fnv1a(b"cho"), 0xf5faf0190cf90df3);
        assert_eq!(fnv1a(b"chon"), 0xf27397910b3221c7);
        assert_eq!(fnv1a(b"chong"), 0x2c8c2b76062f22e0);
        assert_eq!(fnv1a(b"chongo"), 0xe150688c8217b8fd);
        assert_eq!(fnv1a(b"chongo "), 0xf35a83c10e4f1f87);
        assert_eq!(fnv1a(b"chongo w"), 0xd1edd10b507344d0);
        assert_eq!(fnv1a(b"chongo wa"), 0x2a5ee739b3ddb8c3);
        assert_eq!(fnv1a(b"chongo was"), 0xdcfb970ca1c0d310);
        assert_eq!(fnv1a(b"chongo was "), 0x4054da76daa6da90);
        assert_eq!(fnv1a(b"chongo was h"), 0xf70a2ff589861368);
        assert_eq!(fnv1a(b"chongo was he"), 0x4c628b38aed25f17);
        assert_eq!(fnv1a(b"chongo was her"), 0x9dd1f6510f78189f);
        assert_eq!(fnv1a(b"chongo was here"), 0xa3de85bd491270ce);
        assert_eq!(fnv1a(b"chongo was here!"), 0x858e2fa32a55e61d);
        assert_eq!(fnv1a(b"chongo was here!\n"), 0x46810940eff5f915);
        assert_eq!(fnv1a(b"ch\0"), 0xf5fadd190cf8edaa);
        assert_eq!(fnv1a(b"cho\0"), 0xf273ed910b32b3e9);
        assert_eq!(fnv1a(b"chon\0"), 0x2c8c5276062f6525);
        assert_eq!(fnv1a(b"chong\0"), 0xe150b98c821842a0);
        assert_eq!(fnv1a(b"chongo\0"), 0xf35aa3c10e4f55e7);
        assert_eq!(fnv1a(b"chongo \0"), 0xd1ed680b50729265);
        assert_eq!(fnv1a(b"chongo w\0"), 0x2a5f0639b3dded70);
        assert_eq!(fnv1a(b"chongo wa\0"), 0xdcfbaa0ca1c0f359);
        assert_eq!(fnv1a(b"chongo was\0"), 0x4054ba76daa6a430);
        assert_eq!(fnv1a(b"chongo was \0"), 0xf709c7f5898562b0);
        assert_eq!(fnv1a(b"chongo was h\0"), 0x4c62e638aed2f9b8);
        assert_eq!(fnv1a(b"chongo was he\0"), 0x9dd1a8510f779415);
        assert_eq!(fnv1a(b"chongo was her\0"), 0xa3de2abd4911d62d);
        assert_eq!(fnv1a(b"chongo was here\0"), 0x858e0ea32a55ae0a);
        assert_eq!(fnv1a(b"chongo was here!\0"), 0x46810f40eff60347);
        assert_eq!(fnv1a(b"chongo was here!\n\0"), 0xc33bce57bef63eaf);
        assert_eq!(fnv1a(b"cu"), 0x08a24307b54a0265);
        assert_eq!(fnv1a(b"cur"), 0xf5b9fd190cc18d15);
        assert_eq!(fnv1a(b"curd"), 0x4c968290ace35703);
        assert_eq!(fnv1a(b"curds"), 0x07174bd5c64d9350);
        assert_eq!(fnv1a(b"curds "), 0x5a294c3ff5d18750);
        assert_eq!(fnv1a(b"curds a"), 0x05b3c1aeb308b843);
        assert_eq!(fnv1a(b"curds an"), 0xb92a48da37d0f477);
        assert_eq!(fnv1a(b"curds and"), 0x73cdddccd80ebc49);
        assert_eq!(fnv1a(b"curds and "), 0xd58c4c13210a266b);
        assert_eq!(fnv1a(b"curds and w"), 0xe78b6081243ec194);
        assert_eq!(fnv1a(b"curds and wh"), 0xb096f77096a39f34);
        assert_eq!(fnv1a(b"curds and whe"), 0xb425c54ff807b6a3);
        assert_eq!(fnv1a(b"curds and whey"), 0x23e520e2751bb46e);
        assert_eq!(fnv1a(b"curds and whey\n"), 0x1a0b44ccfe1385ec);
        assert_eq!(fnv1a(b"cu\0"), 0xf5ba4b190cc2119f);
        assert_eq!(fnv1a(b"cur\0"), 0x4c962690ace2baaf);
        assert_eq!(fnv1a(b"curd\0"), 0x0716ded5c64cda19);
        assert_eq!(fnv1a(b"curds\0"), 0x5a292c3ff5d150f0);
        assert_eq!(fnv1a(b"curds \0"), 0x05b3e0aeb308ecf0);
        assert_eq!(fnv1a(b"curds a\0"), 0xb92a5eda37d119d9);
        assert_eq!(fnv1a(b"curds an\0"), 0x73ce41ccd80f6635);
        assert_eq!(fnv1a(b"curds and\0"), 0xd58c2c132109f00b);
        assert_eq!(fnv1a(b"curds and \0"), 0xe78baf81243f47d1);
        assert_eq!(fnv1a(b"curds and w\0"), 0xb0968f7096a2ee7c);
        assert_eq!(fnv1a(b"curds and wh\0"), 0xb425a84ff807855c);
        assert_eq!(fnv1a(b"curds and whe\0"), 0x23e4e9e2751b56f9);
        assert_eq!(fnv1a(b"curds and whey\0"), 0x1a0b4eccfe1396ea);
        assert_eq!(fnv1a(b"curds and whey\n\0"), 0x54abd453bb2c9004);
        assert_eq!(fnv1a(b"hi"), 0x08ba5f07b55ec3da);
        assert_eq!(fnv1a(b"hi\0"), 0x337354193006cb6e);
        assert_eq!(fnv1a(b"hello"), 0xa430d84680aabd0b);
        assert_eq!(fnv1a(b"hello\0"), 0xa9bc8acca21f39b1);
        assert_eq!(fnv1a(b"\xff\x00\x00\x01"), 0x6961196491cc682d);
        assert_eq!(fnv1a(b"\x01\x00\x00\xff"), 0xad2bb1774799dfe9);
        assert_eq!(fnv1a(b"\xff\x00\x00\x02"), 0x6961166491cc6314);
        assert_eq!(fnv1a(b"\x02\x00\x00\xff"), 0x8d1bb3904a3b1236);
        assert_eq!(fnv1a(b"\xff\x00\x00\x03"), 0x6961176491cc64c7);
        assert_eq!(fnv1a(b"\x03\x00\x00\xff"), 0xed205d87f40434c7);
        assert_eq!(fnv1a(b"\xff\x00\x00\x04"), 0x6961146491cc5fae);
        assert_eq!(fnv1a(b"\x04\x00\x00\xff"), 0xcd3baf5e44f8ad9c);
        assert_eq!(fnv1a(b"\x40\x51\x4e\x44"), 0xe3b36596127cd6d8);
        assert_eq!(fnv1a(b"\x44\x4e\x51\x40"), 0xf77f1072c8e8a646);
        assert_eq!(fnv1a(b"\x40\x51\x4e\x4a"), 0xe3b36396127cd372);
        assert_eq!(fnv1a(b"\x4a\x4e\x51\x40"), 0x6067dce9932ad458);
        assert_eq!(fnv1a(b"\x40\x51\x4e\x54"), 0xe3b37596127cf208);
        assert_eq!(fnv1a(b"\x54\x4e\x51\x40"), 0x4b7b10fa9fe83936);
        assert_eq!(fnv1a(b"127.0.0.1"), 0xaabafe7104d914be);
        assert_eq!(fnv1a(b"127.0.0.1\0"), 0xf4d3180b3cde3eda);
        assert_eq!(fnv1a(b"127.0.0.2"), 0xaabafd7104d9130b);
        assert_eq!(fnv1a(b"127.0.0.2\0"), 0xf4cfb20b3cdb5bb1);
        assert_eq!(fnv1a(b"127.0.0.3"), 0xaabafc7104d91158);
        assert_eq!(fnv1a(b"127.0.0.3\0"), 0xf4cc4c0b3cd87888);
        assert_eq!(fnv1a(b"64.81.78.68"), 0xe729bac5d2a8d3a7);
        assert_eq!(fnv1a(b"64.81.78.68\0"), 0x74bc0524f4dfa4c5);
        assert_eq!(fnv1a(b"64.81.78.74"), 0xe72630c5d2a5b352);
        assert_eq!(fnv1a(b"64.81.78.74\0"), 0x6b983224ef8fb456);
        assert_eq!(fnv1a(b"64.81.78.84"), 0xe73042c5d2ae266d);
        assert_eq!(fnv1a(b"64.81.78.84\0"), 0x8527e324fdeb4b37);
        assert_eq!(fnv1a(b"feedface"), 0x0a83c86fee952abc);
        assert_eq!(fnv1a(b"feedface\0"), 0x7318523267779d74);
        assert_eq!(fnv1a(b"feedfacedaffdeed"), 0x3e66d3d56b8caca1);
        assert_eq!(fnv1a(b"feedfacedaffdeed\0"), 0x956694a5c0095593);
        assert_eq!(fnv1a(b"feedfacedeadbeef"), 0xcac54572bb1a6fc8);
        assert_eq!(fnv1a(b"feedfacedeadbeef\0"), 0xa7a4c9f3edebf0d8);
        assert_eq!(fnv1a(b"line 1\nline 2\nline 3"), 0x7829851fac17b143);
        assert_eq!(fnv1a(b"chongo <Landon Curt Noll> /\\../\\"), 0x2c8f4c9af81bcf06);
        assert_eq!(fnv1a(b"chongo <Landon Curt Noll> /\\../\\\0"), 0xd34e31539740c732);
        assert_eq!(fnv1a(b"chongo (Landon Curt Noll) /\\../\\"), 0x3605a2ac253d2db1);
        assert_eq!(fnv1a(b"chongo (Landon Curt Noll) /\\../\\\0"), 0x08c11b8346f4a3c3);
        assert_eq!(fnv1a(b"http://antwrp.gsfc.nasa.gov/apod/astropix.html"), 0x6be396289ce8a6da);
        assert_eq!(fnv1a(b"http://en.wikipedia.org/wiki/Fowler_Noll_Vo_hash"), 0xd9b957fb7fe794c5);
        assert_eq!(fnv1a(b"http://epod.usra.edu/"), 0x05be33da04560a93);
        assert_eq!(fnv1a(b"http://exoplanet.eu/"), 0x0957f1577ba9747c);
        assert_eq!(fnv1a(b"http://hvo.wr.usgs.gov/cam3/"), 0xda2cc3acc24fba57);
        assert_eq!(fnv1a(b"http://hvo.wr.usgs.gov/cams/HMcam/"), 0x74136f185b29e7f0);
        assert_eq!(fnv1a(b"http://hvo.wr.usgs.gov/kilauea/update/deformation.html"), 0xb2f2b4590edb93b2);
        assert_eq!(fnv1a(b"http://hvo.wr.usgs.gov/kilauea/update/images.html"), 0xb3608fce8b86ae04);
        assert_eq!(fnv1a(b"http://hvo.wr.usgs.gov/kilauea/update/maps.html"), 0x4a3a865079359063);
        assert_eq!(fnv1a(b"http://hvo.wr.usgs.gov/volcanowatch/current_issue.html"), 0x5b3a7ef496880a50);
        assert_eq!(fnv1a(b"http://neo.jpl.nasa.gov/risk/"), 0x48fae3163854c23b);
        assert_eq!(fnv1a(b"http://norvig.com/21-days.html"), 0x07aaa640476e0b9a);
        assert_eq!(fnv1a(b"http://primes.utm.edu/curios/home.php"), 0x2f653656383a687d);
        assert_eq!(fnv1a(b"http://slashdot.org/"), 0xa1031f8e7599d79c);
        assert_eq!(fnv1a(b"http://tux.wr.usgs.gov/Maps/155.25-19.5.html"), 0xa31908178ff92477);
        assert_eq!(fnv1a(b"http://volcano.wr.usgs.gov/kilaueastatus.php"), 0x097edf3c14c3fb83);
        assert_eq!(fnv1a(b"http://www.avo.alaska.edu/activity/Redoubt.php"), 0xb51ca83feaa0971b);
        assert_eq!(fnv1a(b"http://www.dilbert.com/fast/"), 0xdd3c0d96d784f2e9);
        assert_eq!(fnv1a(b"http://www.fourmilab.ch/gravitation/orbits/"), 0x86cd26a9ea767d78);
        assert_eq!(fnv1a(b"http://www.fpoa.net/"), 0xe6b215ff54a30c18);
        assert_eq!(fnv1a(b"http://www.ioccc.org/index.html"), 0xec5b06a1c5531093);
        assert_eq!(fnv1a(b"http://www.isthe.com/cgi-bin/number.cgi"), 0x45665a929f9ec5e5);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/bio.html"), 0x8c7609b4a9f10907);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/index.html"), 0x89aac3a491f0d729);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/src/calc/lucas-calc"), 0x32ce6b26e0f4a403);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/astro/venus2004.html"), 0x614ab44e02b53e01);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/astro/vita.html"), 0xfa6472eb6eef3290);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/comp/c/expert.html"), 0x9e5d75eb1948eb6a);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/comp/calc/index.html"), 0xb6d12ad4a8671852);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/comp/fnv/index.html"), 0x88826f56eba07af1);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/math/number/howhigh.html"), 0x44535bf2645bc0fd);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/math/number/number.html"), 0x169388ffc21e3728);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/math/prime/mersenne.html"), 0xf68aac9e396d8224);
        assert_eq!(fnv1a(b"http://www.isthe.com/chongo/tech/math/prime/mersenne.html#largest"), 0x8e87d7e7472b3883);
        assert_eq!(fnv1a(b"http://www.lavarnd.org/cgi-bin/corpspeak.cgi"), 0x295c26caa8b423de);
        assert_eq!(fnv1a(b"http://www.lavarnd.org/cgi-bin/haiku.cgi"), 0x322c814292e72176);
        assert_eq!(fnv1a(b"http://www.lavarnd.org/cgi-bin/rand-none.cgi"), 0x8a06550eb8af7268);
        assert_eq!(fnv1a(b"http://www.lavarnd.org/cgi-bin/randdist.cgi"), 0xef86d60e661bcf71);
        assert_eq!(fnv1a(b"http://www.lavarnd.org/index.html"), 0x9e5426c87f30ee54);
        assert_eq!(fnv1a(b"http://www.lavarnd.org/what/nist-test.html"), 0xf1ea8aa826fd047e);
        assert_eq!(fnv1a(b"http://www.macosxhints.com/"), 0x0babaf9a642cb769);
        assert_eq!(fnv1a(b"http://www.mellis.com/"), 0x4b3341d4068d012e);
        assert_eq!(fnv1a(b"http://www.nature.nps.gov/air/webcams/parks/havoso2alert/havoalert.cfm"), 0xd15605cbc30a335c);
        assert_eq!(fnv1a(b"http://www.nature.nps.gov/air/webcams/parks/havoso2alert/timelines_24.cfm"), 0x5b21060aed8412e5);
        assert_eq!(fnv1a(b"http://www.paulnoll.com/"), 0x45e2cda1ce6f4227);
        assert_eq!(fnv1a(b"http://www.pepysdiary.com/"), 0x50ae3745033ad7d4);
        assert_eq!(fnv1a(b"http://www.sciencenews.org/index/home/activity/view"), 0xaa4588ced46bf414);
        assert_eq!(fnv1a(b"http://www.skyandtelescope.com/"), 0xc1b0056c4a95467e);
        assert_eq!(fnv1a(b"http://www.sput.nl/~rob/sirius.html"), 0x56576a71de8b4089);
        assert_eq!(fnv1a(b"http://www.systemexperts.com/"), 0xbf20965fa6dc927e);
        assert_eq!(fnv1a(b"http://www.tq-international.com/phpBB3/index.php"), 0x569f8383c2040882);
        assert_eq!(fnv1a(b"http://www.travelquesttours.com/index.htm"), 0xe1e772fba08feca0);
        assert_eq!(fnv1a(b"http://www.wunderground.com/global/stations/89606.html"), 0x4ced94af97138ac4);
        assert_eq!(fnv1a(&repeat_10(b"21701")), 0xc4112ffb337a82fb);
        assert_eq!(fnv1a(&repeat_10(b"M21701")), 0xd64a4fd41de38b7d);
        assert_eq!(fnv1a(&repeat_10(b"2^21701-1")), 0x4cfc32329edebcbb);
        assert_eq!(fnv1a(&repeat_10(b"\x54\xc5")), 0x0803564445050395);
        assert_eq!(fnv1a(&repeat_10(b"\xc5\x54")), 0xaa1574ecf4642ffd);
        assert_eq!(fnv1a(&repeat_10(b"23209")), 0x694bc4e54cc315f9);
        assert_eq!(fnv1a(&repeat_10(b"M23209")), 0xa3d7cb273b011721);
        assert_eq!(fnv1a(&repeat_10(b"2^23209-1")), 0x577c2f8b6115bfa5);
        assert_eq!(fnv1a(&repeat_10(b"\x5a\xa9")), 0xb7ec8c1a769fb4c1);
        assert_eq!(fnv1a(&repeat_10(b"\xa9\x5a")), 0x5d5cfce63359ab19);
        assert_eq!(fnv1a(&repeat_10(b"391581216093")), 0x33b96c3cd65b5f71);
        assert_eq!(fnv1a(&repeat_10(b"391581*2^216093-1")), 0xd845097780602bb9);
        assert_eq!(fnv1a(&repeat_10(b"\x05\xf9\x9d\x03\x4c\x81")), 0x84d47645d02da3d5);
        assert_eq!(fnv1a(&repeat_10(b"FEDCBA9876543210")), 0x83544f33b58773a5);
        assert_eq!(fnv1a(&repeat_10(b"\xfe\xdc\xba\x98\x76\x54\x32\x10")), 0x9175cbb2160836c5);
        assert_eq!(fnv1a(&repeat_10(b"EFCDAB8967452301")), 0xc71b3bc175e72bc5);
        assert_eq!(fnv1a(&repeat_10(b"\xef\xcd\xab\x89\x67\x45\x23\x01")), 0x636806ac222ec985);
        assert_eq!(fnv1a(&repeat_10(b"0123456789ABCDEF")), 0xb6ef0e6950f52ed5);
        assert_eq!(fnv1a(&repeat_10(b"\x01\x23\x45\x67\x89\xab\xcd\xef")), 0xead3d8a0f3dfdaa5);
        assert_eq!(fnv1a(&repeat_10(b"1032547698BADCFE")), 0x922908fe9a861ba5);
        assert_eq!(fnv1a(&repeat_10(b"\x10\x32\x54\x76\x98\xba\xdc\xfe")), 0x6d4821de275fd5c5);
        assert_eq!(fnv1a(&repeat_500(b"\x00")), 0x1fe3fce62bd816b5);
        assert_eq!(fnv1a(&repeat_500(b"\x07")), 0xc23e9fccd6f70591);
        assert_eq!(fnv1a(&repeat_500(b"~")), 0xc1af12bdfe16b5b5);
        assert_eq!(fnv1a(&repeat_500(b"\x7f")), 0x39e9f18f2f85e221);
    }
}
