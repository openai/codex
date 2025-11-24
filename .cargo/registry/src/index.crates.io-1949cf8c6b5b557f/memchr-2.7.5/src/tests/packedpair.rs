use alloc::{boxed::Box, vec, vec::Vec};

/// A set of "packed pair" test seeds. Each seed serves as the base for the
/// generation of many other tests. In essence, the seed captures the pair of
/// bytes we used for a predicate and first byte among our needle. The tests
/// generated from each seed essentially vary the length of the needle and
/// haystack, while using the rare/first byte configuration from the seed.
///
/// The purpose of this is to test many different needle/haystack lengths.
/// In particular, some of the vector optimizations might only have bugs
/// in haystacks of a certain size.
const SEEDS: &[Seed] = &[
    // Why not use different 'first' bytes? It seemed like a good idea to be
    // able to configure it, but when I wrote the test generator below, it
    // didn't seem necessary to use for reasons that I forget.
    Seed { first: b'x', index1: b'y', index2: b'z' },
    Seed { first: b'x', index1: b'x', index2: b'z' },
    Seed { first: b'x', index1: b'y', index2: b'x' },
    Seed { first: b'x', index1: b'x', index2: b'x' },
    Seed { first: b'x', index1: b'y', index2: b'y' },
];

/// Runs a host of "packed pair" search tests.
///
/// These tests specifically look for the occurrence of a possible substring
/// match based on a pair of bytes matching at the right offsets.
pub(crate) struct Runner {
    fwd: Option<
        Box<
            dyn FnMut(&[u8], &[u8], u8, u8) -> Option<Option<usize>> + 'static,
        >,
    >,
}

impl Runner {
    /// Create a new test runner for "packed pair" substring search.
    pub(crate) fn new() -> Runner {
        Runner { fwd: None }
    }

    /// Run all tests. This panics on the first failure.
    ///
    /// If the implementation being tested returns `None` for a particular
    /// haystack/needle combination, then that test is skipped.
    ///
    /// This runs tests on both the forward and reverse implementations given.
    /// If either (or both) are missing, then tests for that implementation are
    /// skipped.
    pub(crate) fn run(self) {
        if let Some(mut fwd) = self.fwd {
            for seed in SEEDS.iter() {
                for t in seed.generate() {
                    match fwd(&t.haystack, &t.needle, t.index1, t.index2) {
                        None => continue,
                        Some(result) => {
                            assert_eq!(
                                t.fwd, result,
                                "FORWARD, needle: {:?}, haystack: {:?}, \
                                 index1: {:?}, index2: {:?}",
                                t.needle, t.haystack, t.index1, t.index2,
                            )
                        }
                    }
                }
            }
        }
    }

    /// Set the implementation for forward "packed pair" substring search.
    ///
    /// If the closure returns `None`, then it is assumed that the given
    /// test cannot be applied to the particular implementation and it is
    /// skipped. For example, if a particular implementation only supports
    /// needles or haystacks for some minimum length.
    ///
    /// If this is not set, then forward "packed pair" search is not tested.
    pub(crate) fn fwd(
        mut self,
        search: impl FnMut(&[u8], &[u8], u8, u8) -> Option<Option<usize>> + 'static,
    ) -> Runner {
        self.fwd = Some(Box::new(search));
        self
    }
}

/// A test that represents the input and expected output to a "packed pair"
/// search function. The test should be able to run with any "packed pair"
/// implementation and get the expected output.
struct Test {
    haystack: Vec<u8>,
    needle: Vec<u8>,
    index1: u8,
    index2: u8,
    fwd: Option<usize>,
}

impl Test {
    /// Create a new "packed pair" test from a seed and some given offsets to
    /// the pair of bytes to use as a predicate in the seed's needle.
    ///
    /// If a valid test could not be constructed, then None is returned.
    /// (Currently, we take the approach of massaging tests to be valid
    /// instead of rejecting them outright.)
    fn new(
        seed: Seed,
        index1: usize,
        index2: usize,
        haystack_len: usize,
        needle_len: usize,
        fwd: Option<usize>,
    ) -> Option<Test> {
        let mut index1: u8 = index1.try_into().unwrap();
        let mut index2: u8 = index2.try_into().unwrap();
        // The '#' byte is never used in a haystack (unless we're expecting
        // a match), while the '@' byte is never used in a needle.
        let mut haystack = vec![b'@'; haystack_len];
        let mut needle = vec![b'#'; needle_len];
        needle[0] = seed.first;
        needle[index1 as usize] = seed.index1;
        needle[index2 as usize] = seed.index2;
        // If we're expecting a match, then make sure the needle occurs
        // in the haystack at the expected position.
        if let Some(i) = fwd {
            haystack[i..i + needle.len()].copy_from_slice(&needle);
        }
        // If the operations above lead to rare offsets pointing to the
        // non-first occurrence of a byte, then adjust it. This might lead
        // to redundant tests, but it's simpler than trying to change the
        // generation process I think.
        if let Some(i) = crate::memchr(seed.index1, &needle) {
            index1 = u8::try_from(i).unwrap();
        }
        if let Some(i) = crate::memchr(seed.index2, &needle) {
            index2 = u8::try_from(i).unwrap();
        }
        Some(Test { haystack, needle, index1, index2, fwd })
    }
}

/// Data that describes a single prefilter test seed.
#[derive(Clone, Copy)]
struct Seed {
    first: u8,
    index1: u8,
    index2: u8,
}

impl Seed {
    const NEEDLE_LENGTH_LIMIT: usize = {
        #[cfg(not(miri))]
        {
            33
        }
        #[cfg(miri)]
        {
            5
        }
    };

    const HAYSTACK_LENGTH_LIMIT: usize = {
        #[cfg(not(miri))]
        {
            65
        }
        #[cfg(miri)]
        {
            8
        }
    };

    /// Generate a series of prefilter tests from this seed.
    fn generate(self) -> impl Iterator<Item = Test> {
        let len_start = 2;
        // The iterator below generates *a lot* of tests. The number of
        // tests was chosen somewhat empirically to be "bearable" when
        // running the test suite.
        //
        // We use an iterator here because the collective haystacks of all
        // these test cases add up to enough memory to OOM a conservative
        // sandbox or a small laptop.
        (len_start..=Seed::NEEDLE_LENGTH_LIMIT).flat_map(move |needle_len| {
            let index_start = len_start - 1;
            (index_start..needle_len).flat_map(move |index1| {
                (index1..needle_len).flat_map(move |index2| {
                    (needle_len..=Seed::HAYSTACK_LENGTH_LIMIT).flat_map(
                        move |haystack_len| {
                            Test::new(
                                self,
                                index1,
                                index2,
                                haystack_len,
                                needle_len,
                                None,
                            )
                            .into_iter()
                            .chain(
                                (0..=(haystack_len - needle_len)).flat_map(
                                    move |output| {
                                        Test::new(
                                            self,
                                            index1,
                                            index2,
                                            haystack_len,
                                            needle_len,
                                            Some(output),
                                        )
                                    },
                                ),
                            )
                        },
                    )
                })
            })
        })
    }
}
