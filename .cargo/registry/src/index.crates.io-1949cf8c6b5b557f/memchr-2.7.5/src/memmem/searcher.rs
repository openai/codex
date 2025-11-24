use crate::arch::all::{
    packedpair::{HeuristicFrequencyRank, Pair},
    rabinkarp, twoway,
};

#[cfg(target_arch = "aarch64")]
use crate::arch::aarch64::neon::packedpair as neon;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use crate::arch::wasm32::simd128::packedpair as simd128;
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
use crate::arch::x86_64::{
    avx2::packedpair as avx2, sse2::packedpair as sse2,
};

/// A "meta" substring searcher.
///
/// To a first approximation, this chooses what it believes to be the "best"
/// substring search implemnetation based on the needle at construction time.
/// Then, every call to `find` will execute that particular implementation. To
/// a second approximation, multiple substring search algorithms may be used,
/// depending on the haystack. For example, for supremely short haystacks,
/// Rabin-Karp is typically used.
///
/// See the documentation on `Prefilter` for an explanation of the dispatching
/// mechanism. The quick summary is that an enum has too much overhead and
/// we can't use dynamic dispatch via traits because we need to work in a
/// core-only environment. (Dynamic dispatch works in core-only, but you
/// need `&dyn Trait` and we really need a `Box<dyn Trait>` here. The latter
/// requires `alloc`.) So instead, we use a union and an appropriately paired
/// free function to read from the correct field on the union and execute the
/// chosen substring search implementation.
#[derive(Clone)]
pub(crate) struct Searcher {
    call: SearcherKindFn,
    kind: SearcherKind,
    rabinkarp: rabinkarp::Finder,
}

impl Searcher {
    /// Creates a new "meta" substring searcher that attempts to choose the
    /// best algorithm based on the needle, heuristics and what the current
    /// target supports.
    #[inline]
    pub(crate) fn new<R: HeuristicFrequencyRank>(
        prefilter: PrefilterConfig,
        ranker: R,
        needle: &[u8],
    ) -> Searcher {
        let rabinkarp = rabinkarp::Finder::new(needle);
        if needle.len() <= 1 {
            return if needle.is_empty() {
                trace!("building empty substring searcher");
                Searcher {
                    call: searcher_kind_empty,
                    kind: SearcherKind { empty: () },
                    rabinkarp,
                }
            } else {
                trace!("building one-byte substring searcher");
                debug_assert_eq!(1, needle.len());
                Searcher {
                    call: searcher_kind_one_byte,
                    kind: SearcherKind { one_byte: needle[0] },
                    rabinkarp,
                }
            };
        }
        let pair = match Pair::with_ranker(needle, &ranker) {
            Some(pair) => pair,
            None => return Searcher::twoway(needle, rabinkarp, None),
        };
        debug_assert_ne!(
            pair.index1(),
            pair.index2(),
            "pair offsets should not be equivalent"
        );
        #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
        {
            if let Some(pp) = avx2::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building x86_64 AVX2 substring searcher");
                    let kind = SearcherKind { avx2: pp };
                    Searcher { call: searcher_kind_avx2, kind, rabinkarp }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::avx2(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if let Some(pp) = sse2::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building x86_64 SSE2 substring searcher");
                    let kind = SearcherKind { sse2: pp };
                    Searcher { call: searcher_kind_sse2, kind, rabinkarp }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::sse2(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                // We're pretty unlikely to get to this point, but it is
                // possible to be running on x86_64 without SSE2. Namely, it's
                // really up to the OS whether it wants to support vector
                // registers or not.
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            if let Some(pp) = simd128::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building wasm32 simd128 substring searcher");
                    let kind = SearcherKind { simd128: pp };
                    Searcher { call: searcher_kind_simd128, kind, rabinkarp }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::simd128(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
        #[cfg(target_arch = "aarch64")]
        {
            if let Some(pp) = neon::Finder::with_pair(needle, pair) {
                if do_packed_search(needle) {
                    trace!("building aarch64 neon substring searcher");
                    let kind = SearcherKind { neon: pp };
                    Searcher { call: searcher_kind_neon, kind, rabinkarp }
                } else if prefilter.is_none() {
                    Searcher::twoway(needle, rabinkarp, None)
                } else {
                    let prestrat = Prefilter::neon(pp, needle);
                    Searcher::twoway(needle, rabinkarp, Some(prestrat))
                }
            } else if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
        #[cfg(not(any(
            all(target_arch = "x86_64", target_feature = "sse2"),
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            if prefilter.is_none() {
                Searcher::twoway(needle, rabinkarp, None)
            } else {
                let prestrat = Prefilter::fallback(ranker, pair, needle);
                Searcher::twoway(needle, rabinkarp, prestrat)
            }
        }
    }

    /// Creates a new searcher that always uses the Two-Way algorithm. This is
    /// typically used when vector algorithms are unavailable or inappropriate.
    /// (For example, when the needle is "too long.")
    ///
    /// If a prefilter is given, then the searcher returned will be accelerated
    /// by the prefilter.
    #[inline]
    fn twoway(
        needle: &[u8],
        rabinkarp: rabinkarp::Finder,
        prestrat: Option<Prefilter>,
    ) -> Searcher {
        let finder = twoway::Finder::new(needle);
        match prestrat {
            None => {
                trace!("building scalar two-way substring searcher");
                let kind = SearcherKind { two_way: finder };
                Searcher { call: searcher_kind_two_way, kind, rabinkarp }
            }
            Some(prestrat) => {
                trace!(
                    "building scalar two-way \
                     substring searcher with a prefilter"
                );
                let two_way_with_prefilter =
                    TwoWayWithPrefilter { finder, prestrat };
                let kind = SearcherKind { two_way_with_prefilter };
                Searcher {
                    call: searcher_kind_two_way_with_prefilter,
                    kind,
                    rabinkarp,
                }
            }
        }
    }

    /// Searches the given haystack for the given needle. The needle given
    /// should be the same as the needle that this finder was initialized
    /// with.
    ///
    /// Inlining this can lead to big wins for latency, and #[inline] doesn't
    /// seem to be enough in some cases.
    #[inline(always)]
    pub(crate) fn find(
        &self,
        prestate: &mut PrefilterState,
        haystack: &[u8],
        needle: &[u8],
    ) -> Option<usize> {
        if haystack.len() < needle.len() {
            None
        } else {
            // SAFETY: By construction, we've ensured that the function
            // in `self.call` is properly paired with the union used in
            // `self.kind`.
            unsafe { (self.call)(self, prestate, haystack, needle) }
        }
    }
}

impl core::fmt::Debug for Searcher {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Searcher")
            .field("call", &"<searcher function>")
            .field("kind", &"<searcher kind union>")
            .field("rabinkarp", &self.rabinkarp)
            .finish()
    }
}

/// A union indicating one of several possible substring search implementations
/// that are in active use.
///
/// This union should only be read by one of the functions prefixed with
/// `searcher_kind_`. Namely, the correct function is meant to be paired with
/// the union by the caller, such that the function always reads from the
/// designated union field.
#[derive(Clone, Copy)]
union SearcherKind {
    empty: (),
    one_byte: u8,
    two_way: twoway::Finder,
    two_way_with_prefilter: TwoWayWithPrefilter,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    sse2: crate::arch::x86_64::sse2::packedpair::Finder,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    avx2: crate::arch::x86_64::avx2::packedpair::Finder,
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    simd128: crate::arch::wasm32::simd128::packedpair::Finder,
    #[cfg(target_arch = "aarch64")]
    neon: crate::arch::aarch64::neon::packedpair::Finder,
}

/// A two-way substring searcher with a prefilter.
#[derive(Copy, Clone, Debug)]
struct TwoWayWithPrefilter {
    finder: twoway::Finder,
    prestrat: Prefilter,
}

/// The type of a substring search function.
///
/// # Safety
///
/// When using a function of this type, callers must ensure that the correct
/// function is paired with the value populated in `SearcherKind` union.
type SearcherKindFn = unsafe fn(
    searcher: &Searcher,
    prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize>;

/// Reads from the `empty` field of `SearcherKind` to handle the case of
/// searching for the empty needle. Works on all platforms.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.empty` union field is set.
unsafe fn searcher_kind_empty(
    _searcher: &Searcher,
    _prestate: &mut PrefilterState,
    _haystack: &[u8],
    _needle: &[u8],
) -> Option<usize> {
    Some(0)
}

/// Reads from the `one_byte` field of `SearcherKind` to handle the case of
/// searching for a single byte needle. Works on all platforms.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.one_byte` union field is set.
unsafe fn searcher_kind_one_byte(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    _needle: &[u8],
) -> Option<usize> {
    let needle = searcher.kind.one_byte;
    crate::memchr(needle, haystack)
}

/// Reads from the `two_way` field of `SearcherKind` to handle the case of
/// searching for an arbitrary needle without prefilter acceleration. Works on
/// all platforms.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.two_way` union field is set.
unsafe fn searcher_kind_two_way(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    if rabinkarp::is_fast(haystack, needle) {
        searcher.rabinkarp.find(haystack, needle)
    } else {
        searcher.kind.two_way.find(haystack, needle)
    }
}

/// Reads from the `two_way_with_prefilter` field of `SearcherKind` to handle
/// the case of searching for an arbitrary needle with prefilter acceleration.
/// Works on all platforms.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.two_way_with_prefilter` union
/// field is set.
unsafe fn searcher_kind_two_way_with_prefilter(
    searcher: &Searcher,
    prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    if rabinkarp::is_fast(haystack, needle) {
        searcher.rabinkarp.find(haystack, needle)
    } else {
        let TwoWayWithPrefilter { ref finder, ref prestrat } =
            searcher.kind.two_way_with_prefilter;
        let pre = Pre { prestate, prestrat };
        finder.find_with_prefilter(Some(pre), haystack, needle)
    }
}

/// Reads from the `sse2` field of `SearcherKind` to execute the x86_64 SSE2
/// vectorized substring search implementation.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.sse2` union field is set.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn searcher_kind_sse2(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    let finder = &searcher.kind.sse2;
    if haystack.len() < finder.min_haystack_len() {
        searcher.rabinkarp.find(haystack, needle)
    } else {
        finder.find(haystack, needle)
    }
}

/// Reads from the `avx2` field of `SearcherKind` to execute the x86_64 AVX2
/// vectorized substring search implementation.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.avx2` union field is set.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn searcher_kind_avx2(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    let finder = &searcher.kind.avx2;
    if haystack.len() < finder.min_haystack_len() {
        searcher.rabinkarp.find(haystack, needle)
    } else {
        finder.find(haystack, needle)
    }
}

/// Reads from the `simd128` field of `SearcherKind` to execute the wasm32
/// simd128 vectorized substring search implementation.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.simd128` union field is set.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
unsafe fn searcher_kind_simd128(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    let finder = &searcher.kind.simd128;
    if haystack.len() < finder.min_haystack_len() {
        searcher.rabinkarp.find(haystack, needle)
    } else {
        finder.find(haystack, needle)
    }
}

/// Reads from the `neon` field of `SearcherKind` to execute the aarch64 neon
/// vectorized substring search implementation.
///
/// # Safety
///
/// Callers must ensure that the `searcher.kind.neon` union field is set.
#[cfg(target_arch = "aarch64")]
unsafe fn searcher_kind_neon(
    searcher: &Searcher,
    _prestate: &mut PrefilterState,
    haystack: &[u8],
    needle: &[u8],
) -> Option<usize> {
    let finder = &searcher.kind.neon;
    if haystack.len() < finder.min_haystack_len() {
        searcher.rabinkarp.find(haystack, needle)
    } else {
        finder.find(haystack, needle)
    }
}

/// A reverse substring searcher.
#[derive(Clone, Debug)]
pub(crate) struct SearcherRev {
    kind: SearcherRevKind,
    rabinkarp: rabinkarp::FinderRev,
}

/// The kind of the reverse searcher.
///
/// For the reverse case, we don't do any SIMD acceleration or prefilters.
/// There is no specific technical reason why we don't, but rather don't do it
/// because it's not clear it's worth the extra code to do so. If you have a
/// use case for it, please file an issue.
///
/// We also don't do the union trick as we do with the forward case and
/// prefilters. Basically for the same reason we don't have prefilters or
/// vector algorithms for reverse searching: it's not clear it's worth doing.
/// Please file an issue if you have a compelling use case for fast reverse
/// substring search.
#[derive(Clone, Debug)]
enum SearcherRevKind {
    Empty,
    OneByte { needle: u8 },
    TwoWay { finder: twoway::FinderRev },
}

impl SearcherRev {
    /// Creates a new searcher for finding occurrences of the given needle in
    /// reverse. That is, it reports the last (instead of the first) occurrence
    /// of a needle in a haystack.
    #[inline]
    pub(crate) fn new(needle: &[u8]) -> SearcherRev {
        let kind = if needle.len() <= 1 {
            if needle.is_empty() {
                trace!("building empty reverse substring searcher");
                SearcherRevKind::Empty
            } else {
                trace!("building one-byte reverse substring searcher");
                debug_assert_eq!(1, needle.len());
                SearcherRevKind::OneByte { needle: needle[0] }
            }
        } else {
            trace!("building scalar two-way reverse substring searcher");
            let finder = twoway::FinderRev::new(needle);
            SearcherRevKind::TwoWay { finder }
        };
        let rabinkarp = rabinkarp::FinderRev::new(needle);
        SearcherRev { kind, rabinkarp }
    }

    /// Searches the given haystack for the last occurrence of the given
    /// needle. The needle given should be the same as the needle that this
    /// finder was initialized with.
    #[inline]
    pub(crate) fn rfind(
        &self,
        haystack: &[u8],
        needle: &[u8],
    ) -> Option<usize> {
        if haystack.len() < needle.len() {
            return None;
        }
        match self.kind {
            SearcherRevKind::Empty => Some(haystack.len()),
            SearcherRevKind::OneByte { needle } => {
                crate::memrchr(needle, haystack)
            }
            SearcherRevKind::TwoWay { ref finder } => {
                if rabinkarp::is_fast(haystack, needle) {
                    self.rabinkarp.rfind(haystack, needle)
                } else {
                    finder.rfind(haystack, needle)
                }
            }
        }
    }
}

/// Prefilter controls whether heuristics are used to accelerate searching.
///
/// A prefilter refers to the idea of detecting candidate matches very quickly,
/// and then confirming whether those candidates are full matches. This
/// idea can be quite effective since it's often the case that looking for
/// candidates can be a lot faster than running a complete substring search
/// over the entire input. Namely, looking for candidates can be done with
/// extremely fast vectorized code.
///
/// The downside of a prefilter is that it assumes false positives (which are
/// candidates generated by a prefilter that aren't matches) are somewhat rare
/// relative to the frequency of full matches. That is, if a lot of false
/// positives are generated, then it's possible for search time to be worse
/// than if the prefilter wasn't enabled in the first place.
///
/// Another downside of a prefilter is that it can result in highly variable
/// performance, where some cases are extraordinarily fast and others aren't.
/// Typically, variable performance isn't a problem, but it may be for your use
/// case.
///
/// The use of prefilters in this implementation does use a heuristic to detect
/// when a prefilter might not be carrying its weight, and will dynamically
/// disable its use. Nevertheless, this configuration option gives callers
/// the ability to disable prefilters if you have knowledge that they won't be
/// useful.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum PrefilterConfig {
    /// Never used a prefilter in substring search.
    None,
    /// Automatically detect whether a heuristic prefilter should be used. If
    /// it is used, then heuristics will be used to dynamically disable the
    /// prefilter if it is believed to not be carrying its weight.
    Auto,
}

impl Default for PrefilterConfig {
    fn default() -> PrefilterConfig {
        PrefilterConfig::Auto
    }
}

impl PrefilterConfig {
    /// Returns true when this prefilter is set to the `None` variant.
    fn is_none(&self) -> bool {
        matches!(*self, PrefilterConfig::None)
    }
}

/// The implementation of a prefilter.
///
/// This type encapsulates dispatch to one of several possible choices for a
/// prefilter. Generally speaking, all prefilters have the same approximate
/// algorithm: they choose a couple of bytes from the needle that are believed
/// to be rare, use a fast vector algorithm to look for those bytes and return
/// positions as candidates for some substring search algorithm (currently only
/// Two-Way) to confirm as a match or not.
///
/// The differences between the algorithms are actually at the vector
/// implementation level. Namely, we need different routines based on both
/// which target architecture we're on and what CPU features are supported.
///
/// The straight-forwardly obvious approach here is to use an enum, and make
/// `Prefilter::find` do case analysis to determine which algorithm was
/// selected and invoke it. However, I've observed that this leads to poor
/// codegen in some cases, especially in latency sensitive benchmarks. That is,
/// this approach comes with overhead that I wasn't able to eliminate.
///
/// The second obvious approach is to use dynamic dispatch with traits. Doing
/// that in this context where `Prefilter` owns the selection generally
/// requires heap allocation, and this code is designed to run in core-only
/// environments.
///
/// So we settle on using a union (that's `PrefilterKind`) and a function
/// pointer (that's `PrefilterKindFn`). We select the right function pointer
/// based on which field in the union we set, and that function in turn
/// knows which field of the union to access. The downside of this approach
/// is that it forces us to think about safety, but the upside is that
/// there are some nice latency improvements to benchmarks. (Especially the
/// `memmem/sliceslice/short` benchmark.)
///
/// In cases where we've selected a vector algorithm and the haystack given
/// is too short, we fallback to the scalar version of `memchr` on the
/// `rarest_byte`. (The scalar version of `memchr` is still better than a naive
/// byte-at-a-time loop because it will read in `usize`-sized chunks at a
/// time.)
#[derive(Clone, Copy)]
struct Prefilter {
    call: PrefilterKindFn,
    kind: PrefilterKind,
    rarest_byte: u8,
    rarest_offset: u8,
}

impl Prefilter {
    /// Return a "fallback" prefilter, but only if it is believed to be
    /// effective.
    #[inline]
    fn fallback<R: HeuristicFrequencyRank>(
        ranker: R,
        pair: Pair,
        needle: &[u8],
    ) -> Option<Prefilter> {
        /// The maximum frequency rank permitted for the fallback prefilter.
        /// If the rarest byte in the needle has a frequency rank above this
        /// value, then no prefilter is used if the fallback prefilter would
        /// otherwise be selected.
        const MAX_FALLBACK_RANK: u8 = 250;

        trace!("building fallback prefilter");
        let rarest_offset = pair.index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        let rarest_rank = ranker.rank(rarest_byte);
        if rarest_rank > MAX_FALLBACK_RANK {
            None
        } else {
            let finder = crate::arch::all::packedpair::Finder::with_pair(
                needle,
                pair.clone(),
            )?;
            let call = prefilter_kind_fallback;
            let kind = PrefilterKind { fallback: finder };
            Some(Prefilter { call, kind, rarest_byte, rarest_offset })
        }
    }

    /// Return a prefilter using a x86_64 SSE2 vector algorithm.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[inline]
    fn sse2(finder: sse2::Finder, needle: &[u8]) -> Prefilter {
        trace!("building x86_64 SSE2 prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_sse2,
            kind: PrefilterKind { sse2: finder },
            rarest_byte,
            rarest_offset,
        }
    }

    /// Return a prefilter using a x86_64 AVX2 vector algorithm.
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    #[inline]
    fn avx2(finder: avx2::Finder, needle: &[u8]) -> Prefilter {
        trace!("building x86_64 AVX2 prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_avx2,
            kind: PrefilterKind { avx2: finder },
            rarest_byte,
            rarest_offset,
        }
    }

    /// Return a prefilter using a wasm32 simd128 vector algorithm.
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    #[inline]
    fn simd128(finder: simd128::Finder, needle: &[u8]) -> Prefilter {
        trace!("building wasm32 simd128 prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_simd128,
            kind: PrefilterKind { simd128: finder },
            rarest_byte,
            rarest_offset,
        }
    }

    /// Return a prefilter using a aarch64 neon vector algorithm.
    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn neon(finder: neon::Finder, needle: &[u8]) -> Prefilter {
        trace!("building aarch64 neon prefilter");
        let rarest_offset = finder.pair().index1();
        let rarest_byte = needle[usize::from(rarest_offset)];
        Prefilter {
            call: prefilter_kind_neon,
            kind: PrefilterKind { neon: finder },
            rarest_byte,
            rarest_offset,
        }
    }

    /// Return a *candidate* position for a match.
    ///
    /// When this returns an offset, it implies that a match could begin at
    /// that offset, but it may not. That is, it is possible for a false
    /// positive to be returned.
    ///
    /// When `None` is returned, then it is guaranteed that there are no
    /// matches for the needle in the given haystack. That is, it is impossible
    /// for a false negative to be returned.
    ///
    /// The purpose of this routine is to look for candidate matching positions
    /// as quickly as possible before running a (likely) slower confirmation
    /// step.
    #[inline]
    fn find(&self, haystack: &[u8]) -> Option<usize> {
        // SAFETY: By construction, we've ensured that the function in
        // `self.call` is properly paired with the union used in `self.kind`.
        unsafe { (self.call)(self, haystack) }
    }

    /// A "simple" prefilter that just looks for the occurrence of the rarest
    /// byte from the needle. This is generally only used for very small
    /// haystacks.
    #[inline]
    fn find_simple(&self, haystack: &[u8]) -> Option<usize> {
        // We don't use crate::memchr here because the haystack should be small
        // enough that memchr won't be able to use vector routines anyway. So
        // we just skip straight to the fallback implementation which is likely
        // faster. (A byte-at-a-time loop is only used when the haystack is
        // smaller than `size_of::<usize>()`.)
        crate::arch::all::memchr::One::new(self.rarest_byte)
            .find(haystack)
            .map(|i| i.saturating_sub(usize::from(self.rarest_offset)))
    }
}

impl core::fmt::Debug for Prefilter {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Prefilter")
            .field("call", &"<prefilter function>")
            .field("kind", &"<prefilter kind union>")
            .field("rarest_byte", &self.rarest_byte)
            .field("rarest_offset", &self.rarest_offset)
            .finish()
    }
}

/// A union indicating one of several possible prefilters that are in active
/// use.
///
/// This union should only be read by one of the functions prefixed with
/// `prefilter_kind_`. Namely, the correct function is meant to be paired with
/// the union by the caller, such that the function always reads from the
/// designated union field.
#[derive(Clone, Copy)]
union PrefilterKind {
    fallback: crate::arch::all::packedpair::Finder,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    sse2: crate::arch::x86_64::sse2::packedpair::Finder,
    #[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
    avx2: crate::arch::x86_64::avx2::packedpair::Finder,
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    simd128: crate::arch::wasm32::simd128::packedpair::Finder,
    #[cfg(target_arch = "aarch64")]
    neon: crate::arch::aarch64::neon::packedpair::Finder,
}

/// The type of a prefilter function.
///
/// # Safety
///
/// When using a function of this type, callers must ensure that the correct
/// function is paired with the value populated in `PrefilterKind` union.
type PrefilterKindFn =
    unsafe fn(strat: &Prefilter, haystack: &[u8]) -> Option<usize>;

/// Reads from the `fallback` field of `PrefilterKind` to execute the fallback
/// prefilter. Works on all platforms.
///
/// # Safety
///
/// Callers must ensure that the `strat.kind.fallback` union field is set.
unsafe fn prefilter_kind_fallback(
    strat: &Prefilter,
    haystack: &[u8],
) -> Option<usize> {
    strat.kind.fallback.find_prefilter(haystack)
}

/// Reads from the `sse2` field of `PrefilterKind` to execute the x86_64 SSE2
/// prefilter.
///
/// # Safety
///
/// Callers must ensure that the `strat.kind.sse2` union field is set.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn prefilter_kind_sse2(
    strat: &Prefilter,
    haystack: &[u8],
) -> Option<usize> {
    let finder = &strat.kind.sse2;
    if haystack.len() < finder.min_haystack_len() {
        strat.find_simple(haystack)
    } else {
        finder.find_prefilter(haystack)
    }
}

/// Reads from the `avx2` field of `PrefilterKind` to execute the x86_64 AVX2
/// prefilter.
///
/// # Safety
///
/// Callers must ensure that the `strat.kind.avx2` union field is set.
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
unsafe fn prefilter_kind_avx2(
    strat: &Prefilter,
    haystack: &[u8],
) -> Option<usize> {
    let finder = &strat.kind.avx2;
    if haystack.len() < finder.min_haystack_len() {
        strat.find_simple(haystack)
    } else {
        finder.find_prefilter(haystack)
    }
}

/// Reads from the `simd128` field of `PrefilterKind` to execute the wasm32
/// simd128 prefilter.
///
/// # Safety
///
/// Callers must ensure that the `strat.kind.simd128` union field is set.
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
unsafe fn prefilter_kind_simd128(
    strat: &Prefilter,
    haystack: &[u8],
) -> Option<usize> {
    let finder = &strat.kind.simd128;
    if haystack.len() < finder.min_haystack_len() {
        strat.find_simple(haystack)
    } else {
        finder.find_prefilter(haystack)
    }
}

/// Reads from the `neon` field of `PrefilterKind` to execute the aarch64 neon
/// prefilter.
///
/// # Safety
///
/// Callers must ensure that the `strat.kind.neon` union field is set.
#[cfg(target_arch = "aarch64")]
unsafe fn prefilter_kind_neon(
    strat: &Prefilter,
    haystack: &[u8],
) -> Option<usize> {
    let finder = &strat.kind.neon;
    if haystack.len() < finder.min_haystack_len() {
        strat.find_simple(haystack)
    } else {
        finder.find_prefilter(haystack)
    }
}

/// PrefilterState tracks state associated with the effectiveness of a
/// prefilter. It is used to track how many bytes, on average, are skipped by
/// the prefilter. If this average dips below a certain threshold over time,
/// then the state renders the prefilter inert and stops using it.
///
/// A prefilter state should be created for each search. (Where creating an
/// iterator is treated as a single search.) A prefilter state should only be
/// created from a `Freqy`. e.g., An inert `Freqy` will produce an inert
/// `PrefilterState`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PrefilterState {
    /// The number of skips that has been executed. This is always 1 greater
    /// than the actual number of skips. The special sentinel value of 0
    /// indicates that the prefilter is inert. This is useful to avoid
    /// additional checks to determine whether the prefilter is still
    /// "effective." Once a prefilter becomes inert, it should no longer be
    /// used (according to our heuristics).
    skips: u32,
    /// The total number of bytes that have been skipped.
    skipped: u32,
}

impl PrefilterState {
    /// The minimum number of skip attempts to try before considering whether
    /// a prefilter is effective or not.
    const MIN_SKIPS: u32 = 50;

    /// The minimum amount of bytes that skipping must average.
    ///
    /// This value was chosen based on varying it and checking
    /// the microbenchmarks. In particular, this can impact the
    /// pathological/repeated-{huge,small} benchmarks quite a bit if it's set
    /// too low.
    const MIN_SKIP_BYTES: u32 = 8;

    /// Create a fresh prefilter state.
    #[inline]
    pub(crate) fn new() -> PrefilterState {
        PrefilterState { skips: 1, skipped: 0 }
    }

    /// Update this state with the number of bytes skipped on the last
    /// invocation of the prefilter.
    #[inline]
    fn update(&mut self, skipped: usize) {
        self.skips = self.skips.saturating_add(1);
        // We need to do this dance since it's technically possible for
        // `skipped` to overflow a `u32`. (And we use a `u32` to reduce the
        // size of a prefilter state.)
        self.skipped = match u32::try_from(skipped) {
            Err(_) => core::u32::MAX,
            Ok(skipped) => self.skipped.saturating_add(skipped),
        };
    }

    /// Return true if and only if this state indicates that a prefilter is
    /// still effective.
    #[inline]
    fn is_effective(&mut self) -> bool {
        if self.is_inert() {
            return false;
        }
        if self.skips() < PrefilterState::MIN_SKIPS {
            return true;
        }
        if self.skipped >= PrefilterState::MIN_SKIP_BYTES * self.skips() {
            return true;
        }

        // We're inert.
        self.skips = 0;
        false
    }

    /// Returns true if the prefilter this state represents should no longer
    /// be used.
    #[inline]
    fn is_inert(&self) -> bool {
        self.skips == 0
    }

    /// Returns the total number of times the prefilter has been used.
    #[inline]
    fn skips(&self) -> u32 {
        // Remember, `0` is a sentinel value indicating inertness, so we
        // always need to subtract `1` to get our actual number of skips.
        self.skips.saturating_sub(1)
    }
}

/// A combination of prefilter effectiveness state and the prefilter itself.
#[derive(Debug)]
pub(crate) struct Pre<'a> {
    /// State that tracks the effectiveness of a prefilter.
    prestate: &'a mut PrefilterState,
    /// The actual prefilter.
    prestrat: &'a Prefilter,
}

impl<'a> Pre<'a> {
    /// Call this prefilter on the given haystack with the given needle.
    #[inline]
    pub(crate) fn find(&mut self, haystack: &[u8]) -> Option<usize> {
        let result = self.prestrat.find(haystack);
        self.prestate.update(result.unwrap_or(haystack.len()));
        result
    }

    /// Return true if and only if this prefilter should be used.
    #[inline]
    pub(crate) fn is_effective(&mut self) -> bool {
        self.prestate.is_effective()
    }
}

/// Returns true if the needle has the right characteristics for a vector
/// algorithm to handle the entirety of substring search.
///
/// Vector algorithms can be used for prefilters for other substring search
/// algorithms (like Two-Way), but they can also be used for substring search
/// on their own. When used for substring search, vector algorithms will
/// quickly identify candidate match positions (just like in the prefilter
/// case), but instead of returning the candidate position they will try to
/// confirm the match themselves. Confirmation happens via `memcmp`. This
/// works well for short needles, but can break down when many false candidate
/// positions are generated for large needles. Thus, we only permit vector
/// algorithms to own substring search when the needle is of a certain length.
#[inline]
fn do_packed_search(needle: &[u8]) -> bool {
    /// The minimum length of a needle required for this algorithm. The minimum
    /// is 2 since a length of 1 should just use memchr and a length of 0 isn't
    /// a case handled by this searcher.
    const MIN_LEN: usize = 2;

    /// The maximum length of a needle required for this algorithm.
    ///
    /// In reality, there is no hard max here. The code below can handle any
    /// length needle. (Perhaps that suggests there are missing optimizations.)
    /// Instead, this is a heuristic and a bound guaranteeing our linear time
    /// complexity.
    ///
    /// It is a heuristic because when a candidate match is found, memcmp is
    /// run. For very large needles with lots of false positives, memcmp can
    /// make the code run quite slow.
    ///
    /// It is a bound because the worst case behavior with memcmp is
    /// multiplicative in the size of the needle and haystack, and we want
    /// to keep that additive. This bound ensures we still meet that bound
    /// theoretically, since it's just a constant. We aren't acting in bad
    /// faith here, memcmp on tiny needles is so fast that even in pathological
    /// cases (see pathological vector benchmarks), this is still just as fast
    /// or faster in practice.
    ///
    /// This specific number was chosen by tweaking a bit and running
    /// benchmarks. The rare-medium-needle, for example, gets about 5% faster
    /// by using this algorithm instead of a prefilter-accelerated Two-Way.
    /// There's also a theoretical desire to keep this number reasonably
    /// low, to mitigate the impact of pathological cases. I did try 64, and
    /// some benchmarks got a little better, and others (particularly the
    /// pathological ones), got a lot worse. So... 32 it is?
    const MAX_LEN: usize = 32;
    MIN_LEN <= needle.len() && needle.len() <= MAX_LEN
}
