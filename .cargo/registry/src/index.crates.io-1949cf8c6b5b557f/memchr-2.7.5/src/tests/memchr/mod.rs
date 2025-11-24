use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::ext::Byte;

pub(crate) mod naive;
#[macro_use]
pub(crate) mod prop;

const SEEDS: &'static [Seed] = &[
    Seed { haystack: "a", needles: &[b'a'], positions: &[0] },
    Seed { haystack: "aa", needles: &[b'a'], positions: &[0, 1] },
    Seed { haystack: "aaa", needles: &[b'a'], positions: &[0, 1, 2] },
    Seed { haystack: "", needles: &[b'a'], positions: &[] },
    Seed { haystack: "z", needles: &[b'a'], positions: &[] },
    Seed { haystack: "zz", needles: &[b'a'], positions: &[] },
    Seed { haystack: "zza", needles: &[b'a'], positions: &[2] },
    Seed { haystack: "zaza", needles: &[b'a'], positions: &[1, 3] },
    Seed { haystack: "zzza", needles: &[b'a'], positions: &[3] },
    Seed { haystack: "\x00a", needles: &[b'a'], positions: &[1] },
    Seed { haystack: "\x00", needles: &[b'\x00'], positions: &[0] },
    Seed { haystack: "\x00\x00", needles: &[b'\x00'], positions: &[0, 1] },
    Seed { haystack: "\x00a\x00", needles: &[b'\x00'], positions: &[0, 2] },
    Seed { haystack: "zzzzzzzzzzzzzzzza", needles: &[b'a'], positions: &[16] },
    Seed {
        haystack: "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzza",
        needles: &[b'a'],
        positions: &[32],
    },
    // two needles (applied to memchr2 + memchr3)
    Seed { haystack: "az", needles: &[b'a', b'z'], positions: &[0, 1] },
    Seed { haystack: "az", needles: &[b'a', b'z'], positions: &[0, 1] },
    Seed { haystack: "az", needles: &[b'x', b'y'], positions: &[] },
    Seed { haystack: "az", needles: &[b'a', b'y'], positions: &[0] },
    Seed { haystack: "az", needles: &[b'x', b'z'], positions: &[1] },
    Seed { haystack: "yyyyaz", needles: &[b'a', b'z'], positions: &[4, 5] },
    Seed { haystack: "yyyyaz", needles: &[b'z', b'a'], positions: &[4, 5] },
    // three needles (applied to memchr3)
    Seed {
        haystack: "xyz",
        needles: &[b'x', b'y', b'z'],
        positions: &[0, 1, 2],
    },
    Seed {
        haystack: "zxy",
        needles: &[b'x', b'y', b'z'],
        positions: &[0, 1, 2],
    },
    Seed { haystack: "zxy", needles: &[b'x', b'a', b'z'], positions: &[0, 1] },
    Seed { haystack: "zxy", needles: &[b't', b'a', b'z'], positions: &[0] },
    Seed { haystack: "yxz", needles: &[b't', b'a', b'z'], positions: &[2] },
];

/// Runs a host of substring search tests.
///
/// This has support for "partial" substring search implementations only work
/// for a subset of needles/haystacks. For example, the "packed pair" substring
/// search implementation only works for haystacks of some minimum length based
/// of the pair of bytes selected and the size of the vector used.
pub(crate) struct Runner {
    needle_len: usize,
}

impl Runner {
    /// Create a new test runner for forward and reverse byte search
    /// implementations.
    ///
    /// The `needle_len` given must be at most `3` and at least `1`. It
    /// corresponds to the number of needle bytes to search for.
    pub(crate) fn new(needle_len: usize) -> Runner {
        assert!(needle_len >= 1, "needle_len must be at least 1");
        assert!(needle_len <= 3, "needle_len must be at most 3");
        Runner { needle_len }
    }

    /// Run all tests. This panics on the first failure.
    ///
    /// If the implementation being tested returns `None` for a particular
    /// haystack/needle combination, then that test is skipped.
    pub(crate) fn forward_iter<F>(self, mut test: F)
    where
        F: FnMut(&[u8], &[u8]) -> Option<Vec<usize>> + 'static,
    {
        for seed in SEEDS.iter() {
            if seed.needles.len() > self.needle_len {
                continue;
            }
            for t in seed.generate() {
                let results = match test(t.haystack.as_bytes(), &t.needles) {
                    None => continue,
                    Some(results) => results,
                };
                assert_eq!(
                    t.expected,
                    results,
                    "needles: {:?}, haystack: {:?}",
                    t.needles
                        .iter()
                        .map(|&b| b.to_char())
                        .collect::<Vec<char>>(),
                    t.haystack,
                );
            }
        }
    }

    /// Run all tests in the reverse direction. This panics on the first
    /// failure.
    ///
    /// If the implementation being tested returns `None` for a particular
    /// haystack/needle combination, then that test is skipped.
    pub(crate) fn reverse_iter<F>(self, mut test: F)
    where
        F: FnMut(&[u8], &[u8]) -> Option<Vec<usize>> + 'static,
    {
        for seed in SEEDS.iter() {
            if seed.needles.len() > self.needle_len {
                continue;
            }
            for t in seed.generate() {
                let mut results = match test(t.haystack.as_bytes(), &t.needles)
                {
                    None => continue,
                    Some(results) => results,
                };
                results.reverse();
                assert_eq!(
                    t.expected,
                    results,
                    "needles: {:?}, haystack: {:?}",
                    t.needles
                        .iter()
                        .map(|&b| b.to_char())
                        .collect::<Vec<char>>(),
                    t.haystack,
                );
            }
        }
    }

    /// Run all tests as counting tests. This panics on the first failure.
    ///
    /// That is, this only checks that the number of matches is correct and
    /// not whether the offsets of each match are.
    pub(crate) fn count_iter<F>(self, mut test: F)
    where
        F: FnMut(&[u8], &[u8]) -> Option<usize> + 'static,
    {
        for seed in SEEDS.iter() {
            if seed.needles.len() > self.needle_len {
                continue;
            }
            for t in seed.generate() {
                let got = match test(t.haystack.as_bytes(), &t.needles) {
                    None => continue,
                    Some(got) => got,
                };
                assert_eq!(
                    t.expected.len(),
                    got,
                    "needles: {:?}, haystack: {:?}",
                    t.needles
                        .iter()
                        .map(|&b| b.to_char())
                        .collect::<Vec<char>>(),
                    t.haystack,
                );
            }
        }
    }

    /// Like `Runner::forward`, but for a function that returns only the next
    /// match and not all matches.
    ///
    /// If the function returns `None`, then it is skipped.
    pub(crate) fn forward_oneshot<F>(self, mut test: F)
    where
        F: FnMut(&[u8], &[u8]) -> Option<Option<usize>> + 'static,
    {
        self.forward_iter(move |haystack, needles| {
            let mut start = 0;
            let mut results = vec![];
            while let Some(i) = test(&haystack[start..], needles)? {
                results.push(start + i);
                start += i + 1;
            }
            Some(results)
        })
    }

    /// Like `Runner::reverse`, but for a function that returns only the last
    /// match and not all matches.
    ///
    /// If the function returns `None`, then it is skipped.
    pub(crate) fn reverse_oneshot<F>(self, mut test: F)
    where
        F: FnMut(&[u8], &[u8]) -> Option<Option<usize>> + 'static,
    {
        self.reverse_iter(move |haystack, needles| {
            let mut end = haystack.len();
            let mut results = vec![];
            while let Some(i) = test(&haystack[..end], needles)? {
                results.push(i);
                end = i;
            }
            Some(results)
        })
    }
}

/// A single test for memr?chr{,2,3}.
#[derive(Clone, Debug)]
struct Test {
    /// The string to search in.
    haystack: String,
    /// The needles to look for.
    needles: Vec<u8>,
    /// The offsets that are expected to be found for all needles in the
    /// forward direction.
    expected: Vec<usize>,
}

impl Test {
    fn new(seed: &Seed) -> Test {
        Test {
            haystack: seed.haystack.to_string(),
            needles: seed.needles.to_vec(),
            expected: seed.positions.to_vec(),
        }
    }
}

/// Data that can be expanded into many memchr tests by padding out the corpus.
#[derive(Clone, Debug)]
struct Seed {
    /// The thing to search. We use `&str` instead of `&[u8]` because they
    /// are nicer to write in tests, and we don't miss much since memchr
    /// doesn't care about UTF-8.
    ///
    /// Corpora cannot contain either '%' or '#'. We use these bytes when
    /// expanding test cases into many test cases, and we assume they are not
    /// used. If they are used, `memchr_tests` will panic.
    haystack: &'static str,
    /// The needles to search for. This is intended to be an alternation of
    /// needles. The number of needles may cause this test to be skipped for
    /// some memchr variants. For example, a test with 2 needles cannot be used
    /// to test `memchr`, but can be used to test `memchr2` and `memchr3`.
    /// However, a test with only 1 needle can be used to test all of `memchr`,
    /// `memchr2` and `memchr3`. We achieve this by filling in the needles with
    /// bytes that we never used in the corpus (such as '#').
    needles: &'static [u8],
    /// The positions expected to match for all of the needles.
    positions: &'static [usize],
}

impl Seed {
    /// Controls how much we expand the haystack on either side for each test.
    /// We lower this on Miri because otherwise running the tests would take
    /// forever.
    const EXPAND_LEN: usize = {
        #[cfg(not(miri))]
        {
            515
        }
        #[cfg(miri)]
        {
            6
        }
    };

    /// Expand this test into many variations of the same test.
    ///
    /// In particular, this will generate more tests with larger corpus sizes.
    /// The expected positions are updated to maintain the integrity of the
    /// test.
    ///
    /// This is important in testing a memchr implementation, because there are
    /// often different cases depending on the length of the corpus.
    ///
    /// Note that we extend the corpus by adding `%` bytes, which we
    /// don't otherwise use as a needle.
    fn generate(&self) -> impl Iterator<Item = Test> {
        let mut more = vec![];

        // Add bytes to the start of the corpus.
        for i in 0..Seed::EXPAND_LEN {
            let mut t = Test::new(self);
            let mut new: String = core::iter::repeat('%').take(i).collect();
            new.push_str(&t.haystack);
            t.haystack = new;
            t.expected = t.expected.into_iter().map(|p| p + i).collect();
            more.push(t);
        }
        // Add bytes to the end of the corpus.
        for i in 1..Seed::EXPAND_LEN {
            let mut t = Test::new(self);
            let padding: String = core::iter::repeat('%').take(i).collect();
            t.haystack.push_str(&padding);
            more.push(t);
        }

        more.into_iter()
    }
}
