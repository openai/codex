
Either
======

The enum ``Either`` with variants ``Left`` and ``Right`` and trait
implementations including Iterator, Read, Write.

Either has methods that are similar to Option and Result.

Includes convenience macros ``try_left!()`` and ``try_right!()`` to use for
short-circuiting logic.

Please read the `API documentation here`__

__ https://docs.rs/either/

|build_status|_ |crates|_

.. |build_status| image:: https://github.com/rayon-rs/either/workflows/CI/badge.svg?branch=main
.. _build_status: https://github.com/rayon-rs/either/actions

.. |crates| image:: https://img.shields.io/crates/v/either.svg
.. _crates: https://crates.io/crates/either

How to use with cargo::

    [dependencies]
    either = "1"


Recent Changes
--------------

- 1.15.0

  - Fix ``serde`` support when building without ``std``, by @klkvr (#119)

  - Use a more common ``std`` feature for default enablement, deprecating
    the ``use_std`` feature as a mere alias of the new name.

- 1.14.0

  - **MSRV**: ``either`` now requires Rust 1.63 or later.

  - Implement ``fmt::Write`` for ``Either``, by @yotamofek (#113)

  - Replace ``Into<Result> for Either`` with ``From<Either> for Result``, by @cuviper (#118)

- 1.13.0

  - Add new methods ``.cloned()`` and ``.copied()``, by @ColonelThirtyTwo (#107)

- 1.12.0

  - **MSRV**: ``either`` now requires Rust 1.37 or later.

  - Specialize ``nth_back`` for ``Either`` and ``IterEither``, by @cuviper (#106)

- 1.11.0

  - Add new trait ``IntoEither`` that is useful to convert to ``Either`` in method chains,
    by @SFM61319 (#101)

- 1.10.0

  - Add new methods ``.factor_iter()``, ``.factor_iter_mut()``,  and ``.factor_into_iter()``
    that return ``Either`` items, plus ``.iter()`` and ``.iter_mut()`` to convert to direct
    reference iterators; by @aj-bagwell and @cuviper (#91)

- 1.9.0

  - Add new methods ``.map_either()`` and ``.map_either_with()``, by @nasadorian (#82)

- 1.8.1

  - Clarified that the multiple licenses are combined with OR.

- 1.8.0

  - **MSRV**: ``either`` now requires Rust 1.36 or later.

  - Add new methods ``.as_pin_ref()`` and ``.as_pin_mut()`` to project a
    pinned ``Either`` as inner ``Pin`` variants, by @cuviper (#77)

  - Implement the ``Future`` trait, by @cuviper (#77)

  - Specialize more methods of the ``io`` traits, by @Kixunil and @cuviper (#75)

- 1.7.0

  - **MSRV**: ``either`` now requires Rust 1.31 or later.

  - Export the macro ``for_both!``, by @thomaseizinger (#58)

  - Implement the ``io::Seek`` trait, by @Kerollmops (#60)

  - Add new method ``.either_into()`` for ``Into`` conversion, by @TonalidadeHidrica (#63)

  - Add new methods ``.factor_ok()``, ``.factor_err()``, and ``.factor_none()``,
    by @zachs18 (#67)

  - Specialize ``source`` in the ``Error`` implementation, by @thomaseizinger (#69)

  - Specialize more iterator methods and implement the ``FusedIterator`` trait,
    by @Ten0 (#66) and @cuviper (#71)

  - Specialize ``Clone::clone_from``, by @cuviper (#72)

- 1.6.1

  - Add new methods ``.expect_left()``, ``.unwrap_left()``,
    and equivalents on the right, by @spenserblack (#51)

- 1.6.0

  - Add new modules ``serde_untagged`` and ``serde_untagged_optional`` to customize
    how ``Either`` fields are serialized in other types, by @MikailBag (#49)

- 1.5.3

  - Add new method ``.map()`` for ``Either<T, T>`` by @nvzqz (#40).

- 1.5.2

  - Add new methods ``.left_or()``, ``.left_or_default()``, ``.left_or_else()``,
    and equivalents on the right, by @DCjanus (#36)

- 1.5.1

  - Add ``AsRef`` and ``AsMut`` implementations for common unsized types:
    ``str``, ``[T]``, ``CStr``, ``OsStr``, and ``Path``, by @mexus (#29)

- 1.5.0

  - Add new methods ``.factor_first()``, ``.factor_second()`` and ``.into_inner()``
    by @mathstuf (#19)

- 1.4.0

  - Add inherent method ``.into_iter()`` by @cuviper (#12)

- 1.3.0

  - Add opt-in serde support by @hcpl

- 1.2.0

  - Add method ``.either_with()`` by @Twey (#13)

- 1.1.0

  - Add methods ``left_and_then``, ``right_and_then`` by @rampantmonkey
  - Include license files in the repository and released crate

- 1.0.3

  - Add crate categories

- 1.0.2

  - Forward more ``Iterator`` methods
  - Implement ``Extend`` for ``Either<L, R>`` if ``L, R`` do.

- 1.0.1

  - Fix ``Iterator`` impl for ``Either`` to forward ``.fold()``.

- 1.0.0

  - Add default crate feature ``use_std`` so that you can opt out of linking to
    std.

- 0.1.7

  - Add methods ``.map_left()``, ``.map_right()`` and ``.either()``.
  - Add more documentation

- 0.1.3

  - Implement Display, Error

- 0.1.2

  - Add macros ``try_left!`` and ``try_right!``.

- 0.1.1

  - Implement Deref, DerefMut

- 0.1.0

  - Initial release
  - Support Iterator, Read, Write

License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
https://www.apache.org/licenses/LICENSE-2.0 or the MIT license
https://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.
