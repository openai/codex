# Nucleo


`nucleo` is a highly performant fuzzy matcher written in rust. It aims to fill the same use case as `fzf` and `skim`. Compared to `fzf` `nucleo` has a significantly faster matching algorithm. This mainly makes a difference when matching patterns with low selectivity on many items. An (unscientific) comparison is shown in the benchmark section below.

> Note: If you are looking for a replacement of the `fuzzy-matcher` crate and not a fully managed fuzzy picker, you should use the [`nucleo-matcher`](https://crates.io/crates/nucleo-matcher) crate.

`nucleo` uses the exact **same scoring system as fzf**. That means you should get the same ranking quality (or better) as you are used to from fzf. However, `nucleo` has a more faithful implementation of the Smith-Waterman algorithm which is normally used in DNA sequence alignment (see https://www.cs.cmu.edu/~ckingsf/bioinfo-lectures/gaps.pdf) with two separate matrices (instead of one like fzf). This means that `nucleo` finds the optimal match more often. For example if you match `foo` in `xf foo` `nucleo` will match `x__foo` but `fzf` will match `xf_oo` (you can increase the word length the result will stay the same). The former is the more intuitive match and has a higher score according to the ranking system that both `nucleo` and fzf.

**Compared to `skim`** (and the `fuzzy-matcher` crate) `nucleo` has an even larger performance advantage and is often around **six times faster** (see benchmarks below). Furthermore, the bonus system used by nucleo and fzf is (in my opinion) more consistent/superior. `nucleo` also handles non-ascii text much better. (`skim`s bonus system and even case insensitivity only work for ASCII).

Nucleo also handles Unicode graphemes more correctly. `Fzf` and `skim` both operate on Unicode code points (chars). That means that multi codepoint graphemes can have weird effects (match multiple times, weirdly change the score, ...). `nucleo` will always use the first codepoint of the grapheme for matching instead (and reports grapheme indices, so they can be highlighted correctly). 

## Status

Nucleo is used in the helix-editor and therefore has a large user base with lots of real world testing. The core matcher implementation is considered complete and is unlikely to see major changes. The `nucleo-matcher` crate is finished and ready for widespread use, breaking changes should be very rare (a 1.0 release should not be far away).

While the high level `nucleo` crate also works well (and is also used in helix), there are still additional features that will be added in the future. The high level crate also need better documentation and will likely see a few API changes in the future.

## Benchmarks

> WIP currently more of a demonstration than a comprehensive benchmark suit
> most notably scientific comparisons with `fzf` are missing (a pain because it can't be called as a library)


### Matcher micro benchmarks

Benchmark comparing the runtime of various patterns matched against all files in the source of the linux kernel. Repeat on your system with `BENCHMARK_DIR=<path_to_linux> cargo run -p benches --release` (you can specify an empty directory and the kernel is cloned automatically).

Method                 |     Mean  |    Samples
-----------------------|-----------|-----------
nucleo "never_matches" |  2.30 ms  |2,493/2,500
skim "never_matches"   | 17.44 ms  |    574/574
nucleo "copying"       |  2.12 ms  |2,496/2,500
skim "copying"         | 16.85 ms  |    593/594
nucleo "/doc/kernel"   |  2.59 ms  |2,499/2,500
skim "/doc/kernel"     | 18.32 ms  |    546/546
nucleo "//.h"          |  9.53 ms  |1,049/1,049
skim "//.h"            | 35.46 ms  |    282/282


### Comparison with fzf

For example in the following two screencasts the pattern `///.` is pasted into `fzf` and `nucleo` (both with about 3 million items open).

`fzf` takes a while to filter the text (about 1 second) while `nucleo` has barely any noticeable delay (a single frame in the screencast so about 1/30 seconds). This comparison was made on a very beefy CPU (Ryzen 5950x) so on slower systems the difference may be larger:

[![asciicast](https://asciinema.org/a/600517.svg)](https://asciinema.org/a/600517)
[![asciicast](https://asciinema.org/a/600516.svg)](https://asciinema.org/a/600516)



# Future Work

* [x] merge integration into helix
* [ ] build a standalone CLI application
  * [ ] reach feature parity with `fzf` (mostly `--no-sort` and `--tac`)
  * [ ] add a way to allow columnar matching
* [ ] expose C API so both the high level API and the matching algorithm itself can be used in other applications (like various nvim plugins)

# Naming

The name `nucleo` plays on the fact that the `Smith-Waterman` algorithm (that it's based on) was originally developed for matching DNA/RNA sequences. The elements of DNA/RNA that are matched are called *nucleotides* which was shortened to `nucleo` here.

The name also indicates its close relationship with the *helix* editor (sticking with the DNA theme).

# Implementation Details

> This is only intended for those interested and will not be relevant to most people. I plan to turn this into a blog post when I have more time

<!-- Nucleo matching algorithm has `O(N-M)` space complexity while ranking/filtering (and not computing indices) compared to the `O(MN)` space complexity of fzf. --> 

<!-- Furthermore, `nucleo` also features fully lock-free multithreaded streaming so if used as a library its possible to performantly scale streaming to a practically unlimited number of producer threads (for example running `ignore` or `jwalk` across all cores) without any buffering or other additional logic. -->


The fuzzy matching algorithm is based on the `Smith-Waterman` (with affine gaps) as described in https://www.cs.cmu.edu/~ckingsf/bioinfo-lectures/gaps.pdf (TODO: explain). `Nucleo` faithfully implements this algorithm and therefore has two separate matrices. However, by precomputing the next `m-matrix` row we can avoid storing the p-matrix at all and instead just store the value in a variable as we iterate the row.

Nucleo also never really stores the `m-matrix` instead we only ever store the current row (which simultaneously serves as the next row). During index calculation a full matrix is however required to backtrack which indices were actually matched. We only store two bools here (to indicate where we came from in the matrix).

By comparison `skim` stores the full p and m matrix in that case. `fzf` always allocates a full `mn` matrix (even during matching!).

`nucleo`s' matrix is only width `n-m+1` instead of width `n`. This comes from the observation that the `p` char requires `p-1` chars before it and `m-p` chars after it, so there are always `p-1 + m-p = m+1` chars that can never match the current char. This works especially well with only using a single row because the first relevant char is always at the same position even though it's technically further to the right. This is particularly nice because we precalculate the m-matrix row. The m-matrix is computed from diagonal elements, so the precalculated values stay in the same matrix cell. 

Compared to `skim` nucleo does couple simpler (but arguably even more impactful) optimizations:
* *Presegment Unicode*: Unicode segmentation is somewhat slow and matcher will filter the same elements quite often so only doing it once is nice. It also prevents a very common source of bugs (mixing of char indices which we use here and utf8 indices) and makes the code a lot simpler as a result. Fzf does the same.
* *Aggressive prefiltering*: Especially for ASCII this works very well, but we also do this for Unicode to a lesser extent. This ensures we reject non-matching haystacks as fast as possible. Usually most haystacks will not match when fuzzy matching large lists so having fast path for that case is a huge win.
* *Special-case ASCII*: 90% of practical text is ASCII. ASCII can be stored as bytes instead of `chars`, so cache locality is improved a lot, and we can use `memchar` for superfast prefilters (even case-insensitive prefilter are possible that way)
* *Fallback for very long matches*: We fall back to greedy matcher which runs in `O(N)` (and `O(1)` space complexity) to avoid the `O(mn)` blowup for large matches. This is fzfs old algorithm and yields decent (but not great) results.



<!-- There is a misunderstanding in both `skim` and fzf. Basically what they do is give a bonus to each character (like word boundaries). That makes senes and is reasonable, but the problem is that they use the **maximum bonus** when multiple chars match in sequence. That means that the bonus of a character depends on which characters exactly matched around it. But the fundamental assumption of this algorithm (and why it doesn't require backtracking) is that the score of each character is independent of what other chars matched (this is the difference between the affine gap and the generic gap case shown in the paper too). During fuzzing I found many cases where this mechanism leads to a non-optimal match being reported (so the sort order and fuzzy indices would be wrong). In my testing removing this mechanism and slightly tweaking the bonus calculation results in similar match quality but made sure the algorithm always worked correctly (and removed a bunch of weird edges cases). --> 
  <!-- * [ ] it seems this makes us overemphasize word boundaries for small search strings, this is likely okay as the consecutive bonus wins fairly quickly. Maybe we just do a greedy search for the first 2 chars to reduce visual noise? -->
<!-- * [x] substring/prefix/postfix/exact matcher -->
<!-- * [ ] case mismatch penalty. This doesn't seem like a good idea to me. `FZF` doesn't do this (only skin), smart case should cover most cases. .would be nice for fully case-insensitive matching without smart case like in autocompletion tough. Realistically there won't be more than 3 items that are identical with different casing tough, so I don't think it matters too much. It is a bit annoying to implement since you can no longer pre-normalize queries(or need two queries) :/ -->
<!-- * [ ] high level API (worker thread, query parsing, sorting), in progress -->
  <!-- * apparently sorting is superfast (at most 5% of match time for `nucleo` matcher with a highly selective query, otherwise its completely negligible compared to fuzzy matching). All the bending over backwards `fzf` does (and `skim` copied but way worse) seems a little silly. I think `fzf` does it because go doesn't have a good parallel sort. `Fzf` divides the matches into a couple fairly large chunks and sorts those on each worker thread and then lazily merges the result. That makes the sorting without the merging `Nlog(N/M)` which is basically equivalent for large `N` and small `M` as is the case here. At least its parallel tough. In rust we have a great pattern defeating parallel quicksort tough (rayon) which is way easier. -->
  <!-- * [x] basic implementation (workers, streaming, invalidation) -->
  <!-- * [x] verify it actually works -->
  <!-- * [x] query paring -->
  <!-- * [x] hook up to helix -->
  <!-- * [x] currently I simply use a tick system (called on every redraw), together with a redraw/tick nofication (ideally debounced) is that enough? yes works nicely -->
  <!-- * [x] for streaming callers should buffer their data. Can we provide a better API for that beyond what is currently there? yes lock-free stream -->
  <!-- * [ ] cleanup code, improve API -->
  <!-- * [ ] write docs -->

<!-- * tests -->
  <!-- * [x] fuzz the fuzzy matcher -->
  <!-- * [x] port the full `fzf` test suite for fuzzy matching -->
  <!-- * [ ] port the full `skim` test suite for fuzzy matching -->
  <!-- * [ ] highlevel API -->
  <!-- * [~] test substring/exact/prefix/postfix match -->
  <!-- * [ ] coverage report (fuzzy matcher was at 86%) -->

