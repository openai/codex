# Changelog

## 0.10.4
  - Add `EitherOrBoth::or` and `EitherOrBoth::or_else` (#593)
  - Add `min_set`, `max_set` et al. (#613, #323)
  - Use `either/use_std` (#628)
  - Documentation fixes (#612, #625, #632, #633, #634, #638)
  - Code maintenance (#623, #624, #627, #630)

## 0.10.2
  - Add `Itertools::multiunzip` (#362, #565)
  - Add `intersperse` and `intersperse_with` free functions (#555)
  - Add `Itertools::sorted_by_cached_key` (#424, #575)
  - Specialize `ProcessResults::fold` (#563)
  - Fix subtraction overflow in `DuplicatesBy::size_hint` (#552)
  - Fix specialization tests (#574)
  - More `Debug` impls (#573)
  - Deprecate `fold1` (use `reduce` instead) (#580)
  - Documentation fixes (`HomogenousTuple`, `into_group_map`, `into_group_map_by`, `MultiPeek::peek`) (#543 et al.)

## 0.10.1
  - Add `Itertools::contains` (#514)
  - Add `Itertools::counts_by` (#515)
  - Add `Itertools::partition_result` (#511)
  - Add `Itertools::all_unique` (#241)
  - Add `Itertools::duplicates` and `Itertools::duplicates_by` (#502)
  - Add `chain!` (#525)
  - Add `Itertools::at_most_one` (#523)
  - Add `Itertools::flatten_ok` (#527)
  - Add `EitherOrBoth::or_default` (#583)
  - Add `Itertools::find_or_last` and `Itertools::find_or_first` (#535)
  - Implement `FusedIterator` for `FilterOk`, `FilterMapOk`, `InterleaveShortest`, `KMergeBy`, `MergeBy`, `PadUsing`, `Positions`, `Product` , `RcIter`, `TupleWindows`, `Unique`, `UniqueBy`,  `Update`, `WhileSome`, `Combinations`, `CombinationsWithReplacement`, `Powerset`, `RepeatN`, and `WithPosition` (#550)
  - Implement `FusedIterator` for `Interleave`, `IntersperseWith`, and `ZipLongest` (#548)

## 0.10.0
  - **Increase minimum supported Rust version to 1.32.0**
  - Improve macro hygiene (#507)
  - Add `Itertools::powerset` (#335)
  - Add `Itertools::sorted_unstable`, `Itertools::sorted_unstable_by`, and `Itertools::sorted_unstable_by_key` (#494)
  - Implement `Error` for `ExactlyOneError` (#484)
  - Undeprecate `Itertools::fold_while` (#476)
  - Tuple-related adapters work for tuples of arity up to 12 (#475)
  - `use_alloc` feature for users who have `alloc`, but not `std` (#474)
  - Add `Itertools::k_smallest` (#473)
  - Add `Itertools::into_grouping_map` and `GroupingMap` (#465)
  - Add `Itertools::into_grouping_map_by` and `GroupingMapBy` (#465)
  - Add `Itertools::counts` (#468)
  - Add implementation of `DoubleEndedIterator` for `Unique` (#442)
  - Add implementation of `DoubleEndedIterator` for `UniqueBy` (#442)
  - Add implementation of `DoubleEndedIterator` for `Zip` (#346)
  - Add `Itertools::multipeek` (#435)
  - Add `Itertools::dedup_with_count` and `DedupWithCount` (#423)
  - Add `Itertools::dedup_by_with_count` and `DedupByWithCount` (#423)
  - Add `Itertools::intersperse_with` and `IntersperseWith` (#381)
  - Add `Itertools::filter_ok` and `FilterOk` (#377)
  - Add `Itertools::filter_map_ok` and `FilterMapOk` (#377)
  - Deprecate `Itertools::fold_results`, use `Itertools::fold_ok` instead (#377)
  - Deprecate `Itertools::map_results`, use `Itertools::map_ok` instead (#377)
  - Deprecate `FoldResults`, use `FoldOk` instead (#377)
  - Deprecate `MapResults`, use `MapOk` instead (#377)
  - Add `Itertools::circular_tuple_windows` and `CircularTupleWindows` (#350)
  - Add `peek_nth` and `PeekNth` (#303)

## 0.9.0
  - Fix potential overflow in `MergeJoinBy::size_hint` (#385)
  - Add `derive(Clone)` where possible (#382)
  - Add `try_collect` method (#394)
  - Add `HomogeneousTuple` trait (#389)
  - Fix `combinations(0)` and `combinations_with_replacement(0)` (#383)
  - Don't require `ParitalEq` to the `Item` of `DedupBy` (#397)
  - Implement missing specializations on the `PutBack` adaptor and on the `MergeJoinBy` iterator (#372)
  - Add `position_*` methods (#412)
  - Derive `Hash` for `EitherOrBoth` (#417)
  - Increase minimum supported Rust version to 1.32.0

## 0.8.2
  - Use `slice::iter` instead of `into_iter` to avoid future breakage (#378, by @LukasKalbertodt)
## 0.8.1
  - Added a [`.exactly_one()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.exactly_one) iterator method that, on success, extracts the single value of an iterator ; by @Xaeroxe
  - Added combinatory iterator adaptors:
    - [`.permutations(k)`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.permutations):

      `[0, 1, 2].iter().permutations(2)` yields

      ```rust
      [
        vec![0, 1],
        vec![0, 2],
        vec![1, 0],
        vec![1, 2],
        vec![2, 0],
        vec![2, 1],
      ]
      ```

      ; by @tobz1000

    - [`.combinations_with_replacement(k)`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.combinations_with_replacement):

      `[0, 1, 2].iter().combinations_with_replacement(2)` yields

      ```rust
      [
        vec![0, 0],
        vec![0, 1],
        vec![0, 2],
        vec![1, 1],
        vec![1, 2],
        vec![2, 2],
      ]
      ```

      ; by @tommilligan

    - For reference, these methods join the already existing [`.combinations(k)`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.combinations):

      `[0, 1, 2].iter().combinations(2)` yields

      ```rust
      [
        vec![0, 1],
        vec![0, 2],
        vec![1, 2],
      ]
      ```

  - Improved the performance of [`.fold()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.fold)-based internal iteration for the [`.intersperse()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.intersperse) iterator ; by @jswrenn
  - Added [`.dedup_by()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.dedup_by), [`.merge_by()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.merge_by) and [`.kmerge_by()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.kmerge_by) adaptors that work like [`.dedup()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.dedup), [`.merge()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.merge) and [`.kmerge()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.kmerge), but taking an additional custom comparison closure parameter. ; by @phimuemue
  - Improved the performance of [`.all_equal()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.all_equal) ; by @fyrchik
  - Loosened the bounds on [`.partition_map()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.partition_map) to take just a `FnMut` closure rather than a `Fn` closure, and made its implementation use internal iteration for better performance ; by @danielhenrymantilla
  - Added convenience methods to [`EitherOrBoth`](https://docs.rs/itertools/0.8.1/itertools/enum.EitherOrBoth.html) elements yielded from the [`.zip_longest()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.zip_longest) iterator adaptor ; by @Avi-D-coder
  - Added [`.sum1()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.sum1) and [`.product1()`](https://docs.rs/itertools/0.8.1/itertools/trait.Itertools.html#method.product1) iterator methods that respectively try to return the sum and the product of the elements of an iterator **when it is not empty**, otherwise they return `None` ; by @Emerentius
## 0.8.0
  - Added new adaptor `.map_into()` for conversions using `Into` by @vorner
  - Improved `Itertools` docs by @JohnHeitmann
  - The return type of `.sorted_by_by_key()` is now an iterator, not a Vec.
  - The return type of the `izip!(x, y)` macro with exactly two arguments is now the usual `Iterator::zip`.
  - Remove `.flatten()` in favour of std's `.flatten()`
  - Deprecate `.foreach()` in favour of std's `.for_each()`
  - Deprecate `.step()` in favour of std's `.step_by()`
  - Deprecate `repeat_call` in favour of std's `repeat_with`
  - Deprecate `.fold_while()` in favour of std's `.try_fold()`
  - Require Rust 1.24 as minimal version.
## 0.7.11
  - Add convenience methods to `EitherOrBoth`, making it more similar to `Option` and `Either` by @jethrogb
## 0.7.10
  - No changes.
## 0.7.9
  - New inclusion policy: See the readme about suggesting features for std before accepting them in itertools.
  - The `FoldWhile` type now implements `Eq` and `PartialEq` by @jturner314
## 0.7.8
  - Add new iterator method `.tree_fold1()` which is like `.fold1()` except items are combined in a tree structure (see its docs). By @scottmcm
  - Add more `Debug` impls by @phimuemue: KMerge, KMergeBy, MergeJoinBy, ConsTuples, Intersperse, ProcessResults, RcIter, Tee, TupleWindows, Tee, ZipLongest, ZipEq, Zip.
## 0.7.7
  - Add new iterator method `.into_group_map() -> HashMap<K, Vec<V>>` which turns an iterator of `(K, V)` elements into such a hash table, where values are grouped by key. By @tobz1000
  - Add new free function `flatten` for the `.flatten()` adaptor. **NOTE:** recent Rust nightlies have `Iterator::flatten` and thus a clash with our flatten adaptor. One workaround is to use the itertools `flatten` free function.
## 0.7.6
  - Add new adaptor `.multi_cartesian_product()` which is an n-ary product iterator by @tobz1000
  - Add new method `.sorted_by_key()` by @Xion
  - Provide simpler and faster `.count()` for `.unique()` and `.unique_by()`
## 0.7.5
  - `.multipeek()` now implements `PeekingNext`, by @nicopap.
## 0.7.4
  - Add new adaptor `.update()` by @lucasem; this adaptor is used to modify an element before passing it on in an iterator chain.
## 0.7.3
  - Add new method `.collect_tuple()` by @matklad; it makes a tuple out of the iterator's elements if the number of them matches **exactly**.
  - Implement `fold` and `collect` for `.map_results()` which means it reuses the code of the standard `.map()` for these methods.
## 0.7.2
  - Add new adaptor `.merge_join_by` by @srijs; a heterogeneous merge join for two ordered sequences.
## 0.7.1
  - Iterator adaptors and iterators in itertools now use the same `must_use` reminder that the standard library adaptors do, by @matematikaedit and @bluss *“iterator adaptors are lazy and do nothing unless consumed”*.
## 0.7.0
  - Faster `izip!()` by @krdln
    - `izip!()` is now a wrapper for repeated regular `.zip()` and a single `.map()`. This means it optimizes as well as the standard library `.zip()` it uses. **Note:** `multizip` and `izip!()` are now different! The former has a named type but the latter optimizes better.
  - Faster `.unique()`
  - `no_std` support, which is opt-in!
    - Many lovable features are still there without std, like `izip!()` or `.format()` or `.merge()`, but not those that use collections.
  - Trait bounds were required up front instead of just on the type: `group_by`'s `PartialEq` by @Phlosioneer and `repeat_call`'s `FnMut`.
  - Removed deprecated constructor `Zip::new` — use `izip!()` or `multizip()`
## 0.6.5
  - Fix bug in `.cartesian_product()`'s fold (which only was visible for unfused iterators).
## 0.6.4
  - Add specific `fold` implementations for `.cartesian_product()` and `cons_tuples()`, which improves their performance in fold, foreach, and iterator consumers derived from them.
## 0.6.3
  - Add iterator adaptor `.positions(predicate)` by @tmccombs
## 0.6.2
  - Add function `process_results` which can “lift” a function of the regular values of an iterator so that it can process the `Ok` values from an iterator of `Results` instead, by @shepmaster
  - Add iterator method `.concat()` which combines all iterator elements into a single collection using the `Extend` trait, by @srijs
## 0.6.1
  - Better size hint testing and subsequent size hint bugfixes by @rkarp. Fixes bugs in product, `interleave_shortest` size hints.
  - New iterator method `.all_equal()` by @phimuemue
## 0.6.0
  - Deprecated names were removed in favour of their replacements
  - `.flatten()` does not implement double ended iteration anymore
  - `.fold_while()` uses `&mut self` and returns `FoldWhile<T>`, for composability #168
  - `.foreach()` and `.fold1()` use `self`, like `.fold()` does.
  - `.combinations(0)` now produces a single empty vector. #174
## 0.5.10
  - Add itertools method `.kmerge_by()` (and corresponding free function)
  - Relaxed trait requirement of `.kmerge()` and `.minmax()` to PartialOrd.
## 0.5.9
  - Add multipeek method `.reset_peek()`
  - Add categories
## 0.5.8
  - Add iterator adaptor `.peeking_take_while()` and its trait `PeekingNext`.
## 0.5.7
  - Add iterator adaptor `.with_position()`
  - Fix multipeek's performance for long peeks by using `VecDeque`.
## 0.5.6
  - Add `.map_results()`
## 0.5.5
  - Many more adaptors now implement `Debug`
  - Add free function constructor `repeat_n`. `RepeatN::new` is now deprecated.
## 0.5.4
  - Add infinite generator function `iterate`, that takes a seed and a closure.
## 0.5.3
  - Special-cased `.fold()` for flatten and put back. `.foreach()` now uses fold on the iterator, to pick up any iterator specific loop implementation.
  - `.combinations(n)` asserts up front that `n != 0`, instead of running into an error on the second iterator element.
## 0.5.2
  - Add `.tuples::<T>()` that iterates by two, three or four elements at a time (where `T` is a tuple type).
  - Add `.tuple_windows::<T>()` that iterates using a window of the two, three or four most recent elements.
  - Add `.next_tuple::<T>()` method, that picks the next two, three or four elements in one go.
  - `.interleave()` now has an accurate size hint.
## 0.5.1
  - Workaround module/function name clash that made racer crash on completing itertools. Only internal changes needed.
## 0.5.0
  - [Release announcement](https://bluss.github.io/rust/2016/09/26/itertools-0.5.0/)
  - Renamed:
    - `combinations` is now `tuple_combinations`
    - `combinations_n` to `combinations`
    - `group_by_lazy`, `chunks_lazy` to `group_by`, `chunks`
    - `Unfold::new` to `unfold()`
    - `RepeatCall::new` to `repeat_call()`
    - `Zip::new` to `multizip`
    - `PutBack::new`, `PutBackN::new` to `put_back`, `put_back_n`
    - `PutBack::with_value` is now a builder setter, not a constructor
    - `MultiPeek::new`, `.multipeek()` to `multipeek()`
    - `format` to `format_with` and `format_default` to `format`
    - `.into_rc()` to `rciter`
    - `Partition` enum is now `Either`
  - Module reorganization:
    - All iterator structs are under `itertools::structs` but also reexported to the top level, for backwards compatibility
    - All free functions are reexported at the root, `itertools::free` will be removed in the next version
  - Removed:
    - `ZipSlices`, use `.zip()` instead
    - `.enumerate_from()`, `ZipTrusted`, due to being unstable
    - `.mend_slices()`, moved to crate `odds`
    - Stride, StrideMut, moved to crate `odds`
    - `linspace()`, moved to crate `itertools-num`
    - `.sort_by()`, use `.sorted_by()`
    - `.is_empty_hint()`, use `.size_hint()`
    - `.dropn()`, use `.dropping()`
    - `.map_fn()`, use `.map()`
    - `.slice()`, use `.take()` / `.skip()`
    - helper traits in `misc`
    - `new` constructors on iterator structs, use `Itertools` trait or free functions instead
    - `itertools::size_hint` is now private
  - Behaviour changes:
    - `format` and `format_with` helpers now panic if you try to format them more than once.
    - `repeat_call` is not double ended anymore
  - New features:
    - tuple flattening iterator is constructible with `cons_tuples`
    - itertools reexports `Either` from the `either` crate. `Either<L, R>` is an iterator when `L, R` are.
    - `MinMaxResult` now implements `Copy` and `Clone`
    - `tuple_combinations` supports 1-4 tuples of combinations (previously just 2)
## 0.4.19
  - Add `.minmax_by()`
  - Add `itertools::free::cloned`
  - Add `itertools::free::rciter`
  - Improve `.step(n)` slightly to take advantage of specialized Fuse better.
## 0.4.18
  - Only changes related to the "unstable" crate feature. This feature is more or less deprecated.
    - Use deprecated warnings when unstable is enabled. `.enumerate_from()` will be removed imminently since it's using a deprecated libstd trait.
## 0.4.17
  - Fix bug in `.kmerge()` that caused it to often produce the wrong order #134
## 0.4.16
  - Improve precision of the `interleave_shortest` adaptor's size hint (it is now computed exactly when possible).
## 0.4.15
  - Fixup on top of the workaround in 0.4.14. A function in `itertools::free` was removed by mistake and now it is added back again.
## 0.4.14
  - Workaround an upstream regression in a Rust nightly build that broke compilation of of `itertools::free::{interleave, merge}`
## 0.4.13
  - Add `.minmax()` and `.minmax_by_key()`, iterator methods for finding both minimum and maximum in one scan.
  - Add `.format_default()`, a simpler version of `.format()` (lazy formatting for iterators).
## 0.4.12
  - Add `.zip_eq()`, an adaptor like `.zip()` except it ensures iterators of inequal length don't pass silently (instead it panics).
  - Add `.fold_while()`, an iterator method that is a fold that can short-circuit.
  - Add `.partition_map()`, an iterator method that can separate elements into two collections.
## 0.4.11
  - Add `.get()` for `Stride{,Mut}` and `.get_mut()` for `StrideMut`
## 0.4.10
  - Improve performance of `.kmerge()`
## 0.4.9
  - Add k-ary merge adaptor `.kmerge()`
  - Fix a bug in `.islice()` with ranges `a..b` where a `> b`.
## 0.4.8
  - Implement `Clone`, `Debug` for `Linspace`
## 0.4.7
  - Add function `diff_with()` that compares two iterators
  - Add `.combinations_n()`, an n-ary combinations iterator
  - Add methods `PutBack::with_value` and `PutBack::into_parts`.
## 0.4.6
  - Add method `.sorted()`
  - Add module `itertools::free` with free function variants of common iterator adaptors and methods. For example `enumerate(iterable)`, `rev(iterable)`, and so on.
## 0.4.5
  - Add `.flatten()`
## 0.4.4
  - Allow composing `ZipSlices` with itself
## 0.4.3
  - Write `iproduct!()` as a single expression; this allows temporary values in its arguments.
## 0.4.2
  - Add `.fold_options()`
  - Require Rust 1.1 or later
## 0.4.1
  - Update `.dropping()` to take advantage of `.nth()`
## 0.4.0
  - `.merge()`, `.unique()` and `.dedup()` now perform better due to not using function pointers
  - Add free functions `enumerate()` and `rev()`
  - Breaking changes:
    - Return types of `.merge()` and `.merge_by()` renamed and changed
    - Method `Merge::new` removed
    - `.merge_by()` now takes a closure that returns bool.
    - Return type of `.dedup()` changed
    - Return type of `.mend_slices()` changed
    - Return type of `.unique()` changed
    - Removed function `times()`, struct `Times`: use a range instead
    - Removed deprecated macro `icompr!()`
    - Removed deprecated `FnMap` and method `.fn_map()`: use `.map_fn()`
    - `.interleave_shortest()` is no longer guaranteed to act like fused
## 0.3.25
  - Rename `.sort_by()` to `.sorted_by()`. Old name is deprecated.
  - Fix well-formedness warnings from RFC 1214, no user visible impact
## 0.3.24
  - Improve performance of `.merge()`'s ordering function slightly
## 0.3.23
  - Added `.chunks()`, similar to (and based on) `.group_by_lazy()`.
  - Tweak linspace to match numpy.linspace and make it double ended.
## 0.3.22
  - Added `ZipSlices`, a fast zip for slices
## 0.3.21
  - Remove `Debug` impl for `Format`, it will have different use later
## 0.3.20
  - Optimize `.group_by_lazy()`
## 0.3.19
  - Added `.group_by_lazy()`, a possibly nonallocating group by
  - Added `.format()`, a nonallocating formatting helper for iterators
  - Remove uses of `RandomAccessIterator` since it has been deprecated in Rust.
## 0.3.17
  - Added (adopted) `Unfold` from Rust
## 0.3.16
  - Added adaptors `.unique()`, `.unique_by()`
## 0.3.15
  - Added method `.sort_by()`
## 0.3.14
  - Added adaptor `.while_some()`
## 0.3.13
  - Added adaptor `.interleave_shortest()`
  - Added adaptor `.pad_using()`
## 0.3.11
  - Added `assert_equal` function
## 0.3.10
  - Bugfix `.combinations()` `size_hint`.
## 0.3.8
  - Added source `RepeatCall`
## 0.3.7
  - Added adaptor `PutBackN`
  - Added adaptor `.combinations()`
## 0.3.6
  - Added `itertools::partition`, partition a sequence in place based on a predicate.
  - Deprecate `icompr!()` with no replacement.
## 0.3.5
  - `.map_fn()` replaces deprecated `.fn_map()`.
## 0.3.4
  - `.take_while_ref()` *by-ref adaptor*
  - `.coalesce()` *adaptor*
  - `.mend_slices()` *adaptor*
## 0.3.3
  - `.dropping_back()` *method*
  - `.fold1()` *method*
  - `.is_empty_hint()` *method*
