# Contributing to itertools

We use stable Rust only.
Please check the minimum version of Rust we use in `Cargo.toml`.

_If you are proposing a major change to CI or a new iterator adaptor for this crate,
then **please first file an issue** describing your proposal._
[Usual concerns about new methods](https://github.com/rust-itertools/itertools/issues/413#issuecomment-657670781).

To pass CI tests successfully, your code must be free of "compiler warnings" and "clippy warnings" and be "rustfmt" formatted.

Note that small PRs are easier to review and therefore are more easily merged.

## Write a new method/adaptor for `Itertools` trait
In general, the code logic should be tested with [quickcheck](https://crates.io/crates/quickcheck) tests in `tests/quick.rs`
which allow us to test properties about the code with randomly generated inputs.

### Behind `use_std`/`use_alloc` feature?
If it needs the "std" (such as using hashes) then it should be behind the `use_std` feature,
or if it requires heap allocation (such as using vectors) then it should be behind the `use_alloc` feature.
Otherwise it should be able to run in `no_std` context.

This mostly applies to your new module, each import from it, and to your new `Itertools` method.

### Pick the right receiver
`self`, `&mut self` or `&self`? From [#710](https://github.com/rust-itertools/itertools/pull/710):

- Take by value when:
    - It transfers ownership to another iterator type, such as `filter`, `map`...
    - It consumes the iterator completely, such as `count`, `last`, `max`...
- Mutably borrow when it consumes only part of the iterator, such as `find`, `all`, `try_collect`...
- Immutably borrow when there is no change, such as `size_hint`.

### Laziness
Iterators are [lazy](https://doc.rust-lang.org/std/iter/index.html#laziness):

- structs of iterator adaptors should have `#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]` ;
- structs of iterators should have `#[must_use = "iterators are lazy and do nothing unless consumed"]`.

Those behaviors are **tested** in `tests/laziness.rs`.

## Specialize `Iterator` methods
It might be more performant to specialize some methods.
However, each specialization should be thoroughly tested.

Correctly specializing methods can be difficult, and _we do not require that you do it on your initial PR_.

Most of the time, we want specializations of:

- [`size_hint`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.size_hint):
  It mostly allows allocation optimizations.
  When always exact, it also enables to implement `ExactSizeIterator`.
  See our private module `src/size_hint.rs` for helpers.
- [`fold`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.fold)
  might make iteration faster than calling `next` repeatedly.
- [`count`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.count),
  [`last`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.last),
  [`nth`](https://doc.rust-lang.org/std/iter/trait.Iterator.html#method.nth)
  as we might be able to avoid iterating on every item with `next`.

Additionally,

- `for_each`, `reduce`, `max/min[_by[_key]]` and `partition` all rely on `fold` so you should specialize it instead.
- `all`, `any`, `find`, `find_map`, `cmp`, `partial_cmp`, `eq`, `ne`, `lt`, `le`, `gt`, `ge` and `position` all rely (by default) on `try_fold`
  which we can not specialize on stable rust, so you might want to wait it stabilizes
  or specialize each of them.
- `DoubleEndedIterator::{nth_back, rfold, rfind}`: similar reasoning.

An adaptor might use the inner iterator specializations for its own specializations.

They are **tested** in `tests/specializations.rs` and **benchmarked** in `benches/specializations.rs`
(build those benchmarks is slow so you might want to temporarily remove the ones you do not want to measure).

## Additional implementations
### The [`Debug`](https://doc.rust-lang.org/std/fmt/trait.Debug.html) implementation
All our iterators should implement `Debug`.

When one of the field is not debuggable (such as _functions_), you must not derive `Debug`.
Instead, manually implement it and _ignore this field_ in our helper macro `debug_fmt_fields`.

<details>
<summary>4 examples (click to expand)</summary>

```rust
use std::fmt;

/* ===== Simple derive. ===== */
#[derive(Debug)]
struct Name1<I> {
    iter: I,
}

/* ===== With an unclonable field. ===== */
struct Name2<I, F> {
    iter: I,
    func: F,
}

// No `F: Debug` bound and the field `func` is ignored.
impl<I: fmt::Debug, F> fmt::Debug for Name2<I, F> {
    // it defines the `fmt` function from a struct name and the fields you want to debug.
    debug_fmt_fields!(Name2, iter);
}

/* ===== With an unclonable field, but another bound to add. ===== */
struct Name3<I: Iterator, F> {
    iter: I,
    item: Option<I::Item>,
    func: F,
}

// Same about `F` and `func`, similar about `I` but we must add the `I::Item: Debug` bound.
impl<I: Iterator + fmt::Debug, F> fmt::Debug for Name3<I, F>
where
    I::Item: fmt::Debug,
{
    debug_fmt_fields!(Name3, iter, item);
}

/* ===== With an unclonable field for which we can provide some information. ===== */
struct Name4<I, F> {
    iter: I,
    func: Option<F>,
}

// If ignore a field is not good enough, implement Debug fully manually.
impl<I: fmt::Debug, F> fmt::Debug for Name4<I, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let func = if self.func.is_some() { "Some(_)" } else { "None" };
        f.debug_struct("Name4")
            .field("iter", &self.iter)
            .field("func", &func)
            .finish()
    }
}
```
</details>

### When/How to implement [`Clone`](https://doc.rust-lang.org/std/clone/trait.Clone.html)
All our iterators should implement `Clone` when possible.

Note that a mutable reference is never clonable so `struct Name<'a, I: 'a> { iter: &'a mut I }` can not implement `Clone`.

Derive `Clone` on a generic struct adds the bound `Clone` on each generic parameter.
It might be an issue in which case you should manually implement it with our helper macro `clone_fields` (it defines the `clone` function calling `clone` on each field) and be careful about the bounds.

### When to implement [`std::iter::FusedIterator`](https://doc.rust-lang.org/std/iter/trait.FusedIterator.html)
This trait should be implemented _by all iterators that always return `None` after returning `None` once_, because it allows to optimize `Iterator::fuse()`.

The conditions on which it should be implemented are usually the ones from the `Iterator` implementation, eventually refined to ensure it behaves in a fused way.

### When to implement [`ExactSizeIterator`](https://doc.rust-lang.org/std/iter/trait.ExactSizeIterator.html)
_When we are always able to return an exact non-overflowing length._

Therefore, we do not implement it on adaptors that makes the iterator longer as the resulting length could overflow.

One should not override `ExactSizeIterator::len` method but rely on an exact `Iterator::size_hint` implementation, meaning it returns `(length, Some(length))` (unless you could make `len` more performant than the default).

The conditions on which it should be implemented are usually the ones from the `Iterator` implementation, probably refined to ensure the size hint is exact.

### When to implement [`DoubleEndedIterator`](https://doc.rust-lang.org/std/iter/trait.DoubleEndedIterator.html)
When the iterator structure allows to handle _iterating on both fronts simultaneously_.
The iteration might stop in the middle when both fronts meet.

The conditions on which it should be implemented are usually the ones from the `Iterator` implementation, probably refined to ensure we can iterate on both fronts simultaneously.

### When to implement [`itertools::PeekingNext`](https://docs.rs/itertools/latest/itertools/trait.PeekingNext.html)
TODO

This is currently **tested** in `tests/test_std.rs`.

## About lending iterators
TODO


## Other notes
No guideline about using `#[inline]` yet.

### `.fold` / `.for_each` / `.try_fold` / `.try_for_each`
In the Rust standard library, it's quite common for `fold` to be implemented in terms of `try_fold`. But it's not something we do yet because we can not specialize `try_fold` methods yet (it uses the unstable `Try`).

From [#781](https://github.com/rust-itertools/itertools/pull/781), the general rule to follow is something like this:

- If you need to completely consume an iterator:
  - Use `fold` if you need an _owned_ access to an accumulator.
  - Use `for_each` otherwise.
- If you need to partly consume an iterator, the same applies with `try_` versions:
  - Use `try_fold` if you need an _owned_ access to an accumulator.
  - Use `try_for_each` otherwise.
