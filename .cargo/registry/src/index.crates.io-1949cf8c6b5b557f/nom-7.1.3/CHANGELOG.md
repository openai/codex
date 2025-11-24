# Change Log

## [Unreleased][unreleased]

### Thanks

### Changed

## 7.1.3 - 2023-01-15

### Thanks

- @Shadow53

### Fixed

- panic in `many` and `count` combinators when the output type is zero sized

## 7.1.2 - 2023-01-01

### Thanks

- @joubs
- @Fyko
- @LoganDark
- @darnuria
- @jkugelman
- @barower
- @puzzlewolf
- @epage
- @cky
- @wolthom
- @w1ll-i-code

### Changed

- documentation fixes
- tests fixes
- limit the initial capacity of the result vector of `many_m_n` to 64kiB
- bits parser now accept `Parser` implementors instead of only functions

### Added

- implement `Tuple` parsing for the unit type as a special case
- implement `ErrorConvert` on the unit type to make it usable as error type for bits parsers
- bool parser for bits input

## 7.1.1 - 2022-03-14

### Thanks

- @ThomasdenH
- @@SphinxKnight
- @irevoire
- @doehyunbaek
- @pxeger
- @punkeel
- @max-sixty
- @Xiretza
- @5c077m4n
- @erihsu
- @TheNeikos
- @LoganDark
- @nickelc
- @chotchki
- @ctrlcctrlv


### Changed

- documentation fixes
- more examples

## 7.1.0 - 2021-11-04

### Thanks

- @nickelc
- @Stargateur
- @NilsIrl
- @clonejo
- @Strytyp
- @schubart
- @jihchi
- @nipunn1313
- @Gungy2
- @Drumato
- @Alexhuszagh
- @Aehmlo
- @homersimpsons
- @dne
- @epage
- @saiintbrisson
- @pymongo

### Changed

- documentation fixes
- Ci fixes
- the move to minimal-lexical for float parsing introduced bugs that cannot be resolved right now, so this version moves back to using the standard lib' parser. *This is a performance regression**. If you have specific requirements around float parsing, you are strongly encouraged to use [recognize_float](https://docs.rs/nom/latest/nom/number/complete/fn.recognize_float.html) and another library to convert to a f32 or f64

### Added

- alt now works with 1 elment tuples

## 7.0.0 - 2021-08-21

This release fixes dependency compilation issues and strengthen the minimum supported Rust version (MSRV) policy. This is also the first release without the macros that were used since nom's beginning.

### Thanks

- @djc
- @homersimpsons
- @lo48576
- @myrrlyn
- @RalXYZ
- @nickelc
- @cenodis

### Added

- `take_until1` combinator
- more `to_owned` implementations
- `fail`: a parser that always fail, useful as default condition in other combinators
- text to number parsers: in the `character::streaming` and `character::complete` modules, there are parsers named `i8, u16, u32, u64, u128` and `u8 ,u16, u32, u64, u128` that recognize decimal digits and directly convert to a number in the target size (checking for max int size)

### Removed

- now that function combinators are the main way to write parsers, the old macro combinators are confusing newcomers. THey have been removed
- the `BitSlice` input type from bitvec has been moved into the [nom-bitvec](https://crates.io/crates/nom-bitvec) crate. nom does not depend on bitvec now
- regex parsers have been moved into the [nom-regex](https://crates.io/crates/nom-regex) crate. nom does not depend on regex now
- `ErrorKind::PArseTo` was not needed anymore

### Changed

- relax trait bounds
- some performance fixes
- `split_at_position*` functions should now be guaranteed panic free
- the `lexical-core` crate used for float parsing has now been replaced with `minimal-lexical`: the new crate is faster to compile, faster to parse, and has no dependencies

### Fixed

- infinite loop in `escaped` combinator
- `many_m_n` now fails if min > max


## 6.2.1 - 2021-06-23

### Thanks

This release was done thanks to the hard work of (by order of appearance in the commit list):

- @homersimpsons

### Fixed

- fix documentation building

## 6.2.0 - 2021-02-15

### Thanks

This release was done thanks to the hard work of (by order of appearance in the commit list):

- @DavidKorczynski
- @homersimpsons
- @kornelski
- @lf-
- @lewisbelcher
- @ronan-d
- @weirane
- @heymind
- @marcianx
- @Nukesor

### Added

- nom is now regularly fuzzed through the OSSFuzz project

### Changed

- lots of documentation fixes
- relax trait bounds
- workarounds for dependency issues with bitvec and memchr

## 6.1.2 - 2021-02-15

### Changed

- Fix cargo feature usage in previous release

## 6.1.1 - 2021-02-15

### Thanks

This release was done thanks to the hard work of (by order of appearance in the commit list):

- @nickelc

### Changed

- Fix dependenciy incompatibilities: Restrict the bitvec->funty dependency to <=1.1

## 6.1.0 - 2021-01-23

### Thanks

This release was done thanks to the hard work of (by order of appearance in the commit list):

- @sachaarbonel
- @vallentin
- @Lucretiel
- @meiomorphism
- @jufajardini
- @neithernut
- @drwilco

### Changed

- readme and documentation fixes
- rewrite of fold_many_m_n
- relax trait bounds on some parsers
- implement `std::error::Error` on `VerboseError`


## 6.0.1 - 2020-11-24

### Thanks

This release was done thanks to the hard work of (by order of appearance in the commit list):

- @Leonqn
- @nickelc
- @toshokan
- @juchiast
- @shssoichiro
- @jlkiri
- @chifflier
- @fkloiber
- @Kaoet
- @Matthew Plant

### Added

- `ErrorConvert` implementation for `VerboseError`

### Changed

- CI fixes
- `fold_many*` now accept `FnMut` for the accumulation function
- relaxed input bounds on `length_count`

# Fixed

- documentation fixes
- the `#[deprecated]` attribute was removed from traits because it does not compile anymore on nightly
- bits and bytes combinators from the bits modules are now converted to use `FnMut`

## 6.0.0 - 2020-10-31

### Thanks

This release was done thanks to the hard work of (by order of appearance in the commit list):
- @chifflier
- @shepmaster
- @amerelo
- @razican
- @Palladinium
- @0ndorio
- Sebastian Zivota
- @keruspe
- @devonhollowood
- @parasyte
- @nnt0
- @AntoineCezar
- @GuillaumeGomez
- @eijebong
- @stadelmanma
- @sphynx
- @snawaz
- @fosskers
- @JamesHarrison
- @calebsander
- @jthornber
- @ahmedcharles
- @rljacobson
- @benkay86
- @georgeclaghorn
- @TianyiShi2001
- @shnewto
- @alfriadox
- @resistor
- @myrrlyn
- @chipsenkbeil
- @ruza-net
- @fanf2
- @jameysharp
- @FallenWarrior2k
- @jmg-duarte
- @ericseppanen
- @hbina
- Andreas Molzer
- @nickelc
- @bgourlie

## Notable changes

This release is a more polished version of nom 5, that came with a focus on
function parsers, by relaxing the requirements: combinators will return a
`impl FnMut` instead of `impl Fn`, allowing closures that change their context,
and parsers can be any type now, as long as they implement the new `Parser` trait.
That parser trait also comes with a few helper methods.

Error management was often a pain point, so a lot of work went into making it easier.
Now it integrates with `std:error::Error`, the `IResult::finish()` method allows you
to convert to a more usable type, the `into` combinator can convert the error type
if there's a `From` implementation, and there are more specific error traits like
`ContextError` for the `context` combinator, and `FromExternalError` for `map_res`.
While the `VerboseError` type and its `convert_error` function saw some changes,
not many features ill be added to it, instead you are encouraged to build the error
type that corresponds to your needs if you are building a language parser.

This version also integrates with the excellent [bitvec](https://crates.io/crates/bitvec)
crate for better bit level parsing. This part of nom was not great and a bit of a hack,
so this will give better options for those parsers.

At last, documentation! There are now more code examples, functions and macros that require
specific cargo features are now clearly indicated, and there's a new `recipes` module
containing example patterns.

### Breaking changes

- the minimal Rust version is now 1.44 (1.37 if building without the `alloc` or `std` features)
- streaming parsers return the number of additional bytes they need, not the total. This was supposed to be the case everywhere, but some parsers were forgotten
- removed the `regexp_macros` cargo feature
- the `context` combinator is not linked to `ParseError` anymore, instead it come with its own `ContextError` trait
- `Needed::Size` now contains a `NonZeroUsize`, so we can reduce the structure's size by 8 bytes. When upgrading, `Needed::Size(number)` can be replaced with `Needed::new(number)`
- there is now a more general `Parser` trait, so parsers can be something else than a function. This trait also comes with combinator methods like `map`, `flat_map`, `or`. Since it is implemented on `Fn*` traits, it should not affect existing code too much
- combinators that returned a `impl Fn` now return a `impl FnMut` to allow parser closures that capture some mutable value from the context
- `separated_list` is now `separated_list0`
- removed the deprecated `methods` module
- removed the deprecated `whitespace` module
- the default error type is now a struct (`nom::error::Error`) instead of a tuple
- the `FromExternalError` allows wrapping the error returned by the function in the `map_res` combinator
- renamed the `dbg!` macro to avoid conflicts with `std::dbg!`
- `separated_list` now allows empty elements


### Added

- function version of regex parsers
- `fill`: attempts to fill the output slice passed as argument
- `success`: returns a value without consuming the input
- `satisfy`: checks a predicate over the next character
- `eof` function combinator
- `consumed`: returns the produced value and the consumed input
- `length_count` function combinator
- `into`: converts a parser's output and error values if `From` implementations are available
- `IResult::finish()`: converts a parser's result to `Result<(I, O), E>` by removing the distinction between `Error` and `Failure` and panicking on `Incomplete`
- non macro versions of `u16`, `i32`, etc, with configurable endianness
- `is_newline` function
- `std::error::Error` implementation for nom's error types
- recipes section of the documentation, outlining common patterns in nom
- custom errors example
- bitstream parsing with the `BitSlice` type from the bitvec crate
- native endianness parsers
- github actions for CI

### Changed

- allows lexical-core 0.7
- number parsers are now generic over the input type
- stabilized the `alloc` feature
- `convert_error` accepts a type that derefs to `&str`
- the JSON example now follows the spec better

### Fixed
- use `fold_many0c` in the `fold_many0` macro

## 5.1.1 - 2020-02-24

### Thanks

- @Alexhuszagh for float fixes
- @AlexanderEkdahl, @JoshOrndorff, @akitsu-sanae for docs fixes
- @ignatenkobrain: dependency update
- @derekdreery: `map` implementation for errors
- @Lucretiel for docs fixes and compilation fixes
- adytzu2007: warning fixes
- @lo48576: error management fixes

### Fixed

- C symbols compilation errors due to old lexical-core version

### Added

- `Err` now has a `map` function

### Changed

- Make `error::context()` available without `alloc` feature

## 5.1.0 - 2020-01-07

### Thanks

- @Hywan, @nickmooney, @jplatte, @ngortheone, @ejmg, @SirWindfield, @demurgos, @spazm, @nyarly, @guedou, @adamnemecek, for docs fixes
- @Alxandr for error management bugfixes
- @Lucretiel for example fixes and optimizations
- @adytzu2007 for optimizations
- @audunhalland for utf8 fixes

### Fixed

- panic in `convert_error`
- `compile_error` macro usage

### Added

- `std::error::Error`, `std::fmt::Display`, `Eq`, `ToOwned` implementations for errors
- inline attribute for  `ToUsize`

### Changed

- `convert_error` optimization
- `alt` optimization

## 5.0.1 - 2019-08-22

### Thanks

- @waywardmonkeys, @phaazon, @dalance for docs fixes
- @kali for `many0_m_n` fixes
- @ia0 for macros fixes

### Fixed

- `many0_m_n` now supports the n=1 case
- relaxed trait requirements in `cut`
- `peek!` macro reimplementation
- type inference in `value!`

## 5.0.0 - 2019-06-24

This version comes with a complete rewrite of nom internals to use functions as a base
for parsers, instead of macros. Macros have been updated to use functions under
the hood, so that most existing parsers will work directly or require minimal changes.

The `CompleteByteSlice` and `CompleteStr` input types were removed. To get different
behaviour related to streaming or complete input, there are different versions of some
parsers in different submodules, like `nom::character::streaming::alpha0` and
`nom::character::complete::alpha0`.

The `verbose-errors` feature is gone, now the error type is decided through a generic
bound. To get equivalent behaviour to `verbose-errors`, check out `nom::error::VerboseError`

### Thanks

- @lowenheim helped in refactoring and error management
- @Keruspe helped in refactoring and fixing tests
- @pingiun, @Songbird0, @jeremystucki, @BeatButton, @NamsooCho, @Waelwindows, @rbtcollins, @MarkMcCaskey for a lot of help in rewriting the documentation and adding code examples
- @GuillaumeGomez for documentation rewriting and checking
- @iosmanthus for bug fixes
- @lo48576 for error management fixes
- @vaffeine for macros visibility fixes
- @webholik and @Havvy for `escaped` and `escaped_transform` fixes
- @proman21 for help on porting bits parsers

### Added

- the `VerboseError` type accumulates position info and error codes, and can generate a trace with span information
- the `lexical-core` crate is now used by default (through the `lexical` compilation feature) to parse floats from text
- documentation and code examples for all functions and macros

### Changed

- nom now uses functions instead of macros to generate parsers
- macros now use the functions under the hood
- the minimal Rust version is now 1.31
- the verify combinator's condition function now takes its argument by reference
- `cond` will now return the error of the parser instead of None
- `alpha*`, `digit*`, `hex_digit*`, `alphanumeric*` now recognize only ASCII characters

### Removed

- deprecated string parsers (with the `_s` suffix), the normal version can be used instead
- `verbose-errors` is not needed anymore, now the error type can be decided when writing the parsers, and parsers provided by nom are generic over the error type
- `AtEof`, `CompleteByteSlice` and `CompleteStr` are gone, instead some parsers are specialized to work on streaming or complete input, and provided in different modules
- character parsers that were aliases to their `*1` version: eol, alpha, digit, hex_digit, oct_digit, alphanumeric, space, multispace
- `count_fixed` macro
- `whitespace::sp` can be replaced by `character::complete::multispace0`
- method combinators are now in the nom-methods crate
- `take_until_either`, `take_until_either1`, `take_until_either_and_consume` and `take_until_either_and_consume1`: they can be replaced with `is_not` (possibly combined with something else)
- `take_until_and_consume`, `take_until_and_consume1`: they can be replaced with `take_until` combined with `take`
- `sized_buffer` and `length_bytes!`: they can be replaced with the `length_data` function
- `non_empty`, `begin` and `rest_s` function
- `cond_reduce!`, `cond_with_error!`, `closure!`, `apply`, `map_res_err!`, `expr_opt!`, `expr_res!`
- `alt_complete`, `separated_list_complete`, `separated_nonempty_list_complete`

## 4.2.3 - 2019-03-23

### Fixed

- add missing `build.rs` file to the package
- fix code comparison links in changelog

## 4.2.2 - 2019-03-04

### Fixed

- regression in do_parse macro import for edition 2018

## 4.2.1 - 2019-02-27

### Fixed

- macro expansion error in `do_parse` due to `compile_error` macro usage

## 4.2.0 - 2019-01-29

### Thanks

- @JoshMcguigan for unit test fixes
- @oza for documentation fixes
- @wackywendell for better error conversion
- @Zebradil for documentation fixes
- @tsraom for new combinators
- @hcpl for minimum Rust version tests
- @KellerFuchs for removing some unsafe uses in float parsing

### Changed

- macro import in edition 2018 code should work without importing internal macros now
- the regex parsers do not require the calling code to have imported the regex crate anymore
- error conversions are more ergonomic
- method combinators are now deprecated. They might be moved to a separate crate
- nom now specifies Rust 1.24.1 as minimum version. This was already the case before, now it is made explicit

### Added

- `many0_count` and `many1_count` to count applications of a parser instead of
accumulating its results in a `Vec`

### Fixed

- overflow in the byte wrapper for bit level parsers
- `f64` parsing does not use `transmute` anymore

## 4.1.1 - 2018-10-14

### Fixed

- compilation issue in verbose-errors mode for `add_return_error`

## 4.1.0 - 2018-10-06

### Thanks

- @xfix for fixing warnings, simplifying examples and performance fixes
- @dvberkel for documentation fixes
- @chifflier for fixing warnings
- @myrrlyn for dead code elimination
- @petrochenkov for removing redundant test macros
- @tbelaire for documentation fixes
- @khernyo for fixing warnings
- @linkmauve for documentation fixes
- @ProgVal for documentation fixes, warning fixes and error management
- @Nemo157 for compilation fixes
- @RReverser for documentation fixes
- @xpayn for fixing warnings
- Blas Rodriguez Irizar for documentation fixes
- @badboy for documentation fixes
- @kyrias for compilation fixes
- @kurnevsky for the `rest_len` parser
- @hjr3 for new documentation examples
- @fengalin for error management
- @ithinuel for the pcap example project
- @phaazon for documentation fixes
- @juchiast for documentation fixes
- @jrakow for the `u128` and `i128` parsers
- @smarnach for documentation fixes
- @derekdreery for `pub(crate)` support
- @YaLTeR for `map_res_err!`

### Added

- `rest_len` parser, returns the length of the remaining input
- `parse_to` has its own error code now
- `u128` and `i128` parsers in big and little endian modes
- support for `pub(crate)` syntax
- `map_res_err!` combinator that appends the error of its argument function in verbose errors mode

### Fixed

- lots of unused imports warnings were removed
- the `bytes` combinator was not compiling in some cases
- the big and little endian combinators now work without external imports
- CI is now faster and uses less cache
- in `add_return_error`, the provided error code is now evaluated only once

### Changed

- `fold_many1` will now transmit a `Failure` instead of transforming it to an `Error`
- `float` and `double` now work on all of nom's input types (`&[u8]`, `&str`, `CompleteByteSlice`, `CompleteStr` and any type that implements the required traits). `float_s` and `double_s` got the same modification, but are now deprecated
- `CompleteByteSlice` and `CompleteStr` get a small optimization by inlining some functions


## 4.0.0 - 2018-05-14

### Thanks

- @jsgf for the new `AtEof` trait
- @tmccombs for fixes on `escaped*` combinators
- @s3bk for fixes around non Copy input types and documentation help
- @kamarkiewicz for fixes to no_std and CI
- @bheisler for documentation and examples
- @target-san for simplifying the `InputIter` trait for `&[u8]`
- @willmurphyscode for documentation and examples
- @Chaitanya1416 for typo fixes
- @fflorent for `input_len()` usage fixes
- @dbrgn for typo fixes
- @iBelieve for no_std fixes
- @kpp for warning fixes and clippy fixes
- @keruspe for fixes on FindToken
- @dtrebbien for fixes on take_until_and_consume1
- @Henning-K for typo fixes
- @vthriller for documentation fixes
- @federicomenaquintero and @veprbl for their help fixing the float parsers
- @vmchale for new named_args versions
- @hywan for documentation fixes
- @fbenkstein for typo fixes
- @CAD97 for catching missing trait implementations
- @goldenlentils for &str optimizations
- @passy for typo fixes
- @ayrat555 for typo fixes
- @GuillaumeGomez for documentation fixes
- @jrakow for documentation fixes and fixes for `switch!`
- @phlosioneer for documentation fixes
- @creativcoder for typo fixes
- @derekdreery for typo fixes
- @lucasem for implementing `Deref` on `CompleteStr` and `CompleteByteSlice`
- @lowenheim for `parse_to!` fixes
- @myrrlyn for trait fixes around `CompleteStr` and `CompleteByteSlice`
- @NotBad4U for fixing code coverage analysis
- @murarth for code formatting
- @glandium for fixing build in no_std
- @csharad for regex compatibility with `CompleteStr`
- @FauxFaux for implementing `AsRef<str>` on `CompleteStr`
- @jaje for implementing `std::Error` on `nom:Err`
- @fengalin for warning fixes
- @@khernyo for doc formatting

Special thanks to @corkami for the logo :)

### Breaking changes

- the `IResult` type now becomes a `Result` from the standard library
- `Incomplete` now returns the additional data size needed, not the total data size needed
- verbose-errors is now a superset of basic errors
- all the errors now include  the related input slice
- the arguments from `error_position` and other such macros were swapped to be more consistent with the rest of nom
- automatic error conversion: to fix error type inference issues, a custom error type must now implement `std::convert::From<u32>`
- the `not!` combinator returns unit `()`
- FindToken's calling convention was swapped
- the `take_*` combinators are now more coherent and stricter, see commit 484f6724ea3ccb for more information
- `many0` and other related parsers will now return `Incomplete` if the reach the end of input without an error of the child parser. They will also return `Incomplete` on an empty input
- the `sep!` combinator for whitespace only consumes whitespace in the prefix, while the `ws!` combinator takes care of consuming the remaining whitespace

### Added

- the `AtEof` trait for input type: indicate if we can get more input data later (related to streaming parsers and `Incomplete` handling)
- the `escaped*` parsers now support the `&str`input type
- the `Failure` error variant represents an unrecoverable error, for which `alt` and other combinators will not try other branches. This error means we got in the right part of the code (like, a prefix was checked correctly), but there was an error in the following parts
- the `CompleteByteSlice` and `CompleteStr` input types consider there will be no more refill of the input. They fixed the `Incomplete` related issues when we have all of the data
- the `exact!()` combinator will fail if we did not consume the whole input
- the `take_while_m_n!` combinator will match a specified number of characters
- `ErrorKind::TakeUntilAndConsume1`
- the `recognize_float` parser will match a float number's characters, but will not transform to a `f32` or `f64`
- `alpha` and other basic parsers are now much stricter about partial inputs. We also introduce the  `*0` and `*1` versions of those parsers
- `named_args` can now specify the input type as well
- `HexDisplay` is now implemented for `&str`
- `alloc` feature
- the `InputTakeAtposition` trait allows specialized implementations of parsers like `take_while!`

### Removed

- the producers and consumers were removed
- the `error_code` and `error_node` macros are not used anymore

### Fixed

- `anychar!` now works correctly with multibyte characters
- `take_until_and_consume1!` no longer results in "no method named \`find_substring\`" and "no method named \`slice\`" compilation errors
- `take_until_and_consume1!` returns the correct Incomplete(Needed) amount
- `no_std` compiles properly, and nom can work with `alloc` too
- `parse_to!` now consumes its input

### Changed

- `alt` and other combinators will now clone the input if necessary. If the input is already `Copy` there is no performance impact
- the `rest` parser now works on various input types
- `InputIter::Item` for `&[u8]` is now a `u8` directly, not a reference
- we now use the `compile_error` macro to return a compile time error if there was a syntax issue
- the permutation combinator now supports optional child parsers
- the float numbers parsers have been refactored to use one common implementation that is nearly 2 times faster than the previous one
- the float number parsers now accept more variants


## 3.2.1 - 2017-10-27

### Thanks

- @ordian for `alt_complete` fixes
- @friedm for documentation fixes
- @kali for improving error management

### Fixed

- there were cases where `alt_complete` could return `Incomplete`

### Added

- an `into_error_kind` method can be used to transform any error to a common value. This helps when the library is included multiple times as dependency with different feature sets


## 3.2.0 - 2017-07-24

### Thanks

- @jedireza for documentation fixes
- @gmorenz for the `bytes` combinator
- @meh for character combinator fixes for UTF-8
- @jethrogb for avoiding move issues in `separated_list`

### Changed

- new layout for the main page of documentation
- `anychar` can now work on any input type
- `length_bytes` is now an alias for `length_data`

### Fixed

- `one_of`, `none_of` and `char` will now index correctly UTF-8 characters
- the `compiler_error` macro is now correctly exported


### Added

- the `bytes` combinator transforms a bit stream back to a byte slice for child parsers

## 3.1.0 - 2017-06-16

### Thanks

- @sdroege: implementing be_i24 and le_i24
- @Hywan: integrating faster substring search using memchr
- @nizox: fixing type issues in bit stream parsing
- @grissiom: documentation fixes
- @doomrobo: implementing separated_list_complete and separated_nonempty_list_complete
- @CWood1: fixing memchr integration in no_std
- @lu_zero: integrating the compiler_error crate
- @dtolnay: helping debug a type inference issue in map

### Changed

- memchr is used for substring search if possible
- if building on nightly, some common syntax errors will display a specific error message. If building no stable, display the documentation to activate those messages
- `count` no longer preallocates its vector

### Fixed

- better type inference in alt_complete
- `alt` should now work with whitespace parsing
- `map` should not make type inference errors anymore

### Added

- be_i24 and le_i24, parsing big endian and little endian signed 24 bit integers
- `separated_list_complete` and `separated_nonempty_list_complete` will treat incomplete from sub parsers as error

## 3.0.0 - 2017-05-12

### Thanks

- Chris Pick for some `Incomplete` related refactors
- @dbrgn for documentation fixes
- @valarauca for adding `be_u24`
- @ithinuel for usability fixes
- @evuez for README readability fixes and improvements to `IResult`
- @s3bk for allowing non-`Copy` types as input
- @keruspe for documentation fixes
- @0xd34d10cc for trait fixes on `InputIter`
- @sdleffler for lifetime shenanigans on `named_args`
- @chengsun for type inference fixes in `alt`
- @iBelieve for adding str to no_std
- @Hywan for simplifying code in input traits
- @azerupi for extensive documentation of `alt` and `alt_complete`

### Breaking Changes

- `escaped`, `separated_list` and `separated_nonempty_list` can now return `Incomplete` when necessary
- `InputIter` does not require `AsChar` on its `Item` type anymore
- the `core` feature that was putting nom in `no_std` mode has been removed. There is now a `std` feature, activated by default. If it is not activated, nom is in `no_std`
- in `verbose-errors` mode, the error list is now stored in a `Vec` instead of a box based linked list
- `chain!` has finally been removed

### Changed

- `Endianness` now implements `Debug`, `PartialEq`, `Eq`, `Clone` and `Copy`
- custom input types can now be cloned if they're not `Copy`
- the infamous 'Cannot infer type for E' error should happen less often now
- `str` is now available in `no_std` mode

### Fixed

- `FileProducer` will be marked as `Eof` on full buffer
- `named_args!` now has lifetimes that cannot conflict with the lifetimes from other arguments

### Added

- `be_u24`: big endian 24 bit unsigned integer parsing
- `IResult` now has a `unwrap_or` method


## 2.2.1 - 2017-04-03

### Thanks

- @Victor-Savu for formatting fixes in the README
- @chifflier for detecting and fixing integer overflows
- @utkarshkukreti for some performance improvements in benchmarks

### Changed

- when calculating how much data is needed in `IResult::Incomplete`, the addition could overflow (it is stored as a usize). This would apparently not result in any security vulnerability on release code

## 2.2.0 - 2017-03-20

### Thanks

- @seppo0010 for fixing `named_args`
- @keruspe for implementing or() on `IResult`, adding the option of default cases in `switch!`, adding support for `cargo-travis`
- @timlyo for documentation fixes
- @JayKickliter for extending `hex_u32`
- @1011X for fixing regex integration
- @Kerollmops for actually marking `chain!` as deprecated
- @joliss for documentation fixes
- @utkarshkukreti for tests refactoring and performance improvement
- @tmccombs for documentation fixes

### Added

- `IResult` gets an `or()` method
- `take_until1`, `take_until_and_consume1`, `take_till1!` and `take_till1_s!` require at least 1 character

### Changed

- `hex_u32` accepts uppercase digits as well
- the character based combinators leverage the input traits
- the whitespace parsers now work on &str and other types
- `take_while1` returns `Incomplete` on empty input
- `switch!` can now take a default case

### Fixed

- `named_args!` now imports `IResult` directly
- the upgrade to regex 0.2 broke the regex combinators, they work now

## 2.1.0 - 2017-01-27

### Thanks

- @nickbabcock for documentation fixes
- @derekdreery for documentation fixes
- @DirkyJerky for documentation fixes
- @saschagrunert for documentation fixes
- @lucab for documentation fixes
- @hyone for documentation fixes
- @tstorch for factoring `Slice`
- @shepmaster for adding crate categories
- @antoyo for adding `named_args!`

### Added

- `verify!` uses a first parser, then applies a function to check that its result satisfies some conditions
- `named_args!` creates a parser function that can accept other arguments along with the input
- `parse_to!` will use the `parse` method from `FromStr` to parse a value. It will automatically translate the input to a string if necessary
- `float`, `float_s`, `double`, `double_s` can recognize floating point numbers in text

### Changed

- `escaped!` will now return `Incomplete` if needed
- `permutation!` supports up to 20 child parsers

## 2.0.1 - 2016-12-10

Bugfix release

*Warning*: there is a small breaking change, `add_error!` is renamed to `add_return_error!`. This was planned for the 2.0 release but was forgotten. This is a small change in a feature that not many people use, for a release that is not yet widely in use, so there will be no 3.0 release for that change.

### Thanks

- @nickbabcock for catching and fixing the `add_error!` mixup
- @lucab for documentation fixes
- @jtdowney for noticing that `tag_no_case!` was not working at all for byte slices

### Fixed

- `add_error!` has been renamed to `add_return_error!`
- the `not!` combinator now accepts functions
- `tag_no_case!` is now working as accepted (before, it accepted everything)


## 2.0 - 2016-11-25

The 2.0 release is one of the biggest yet. It was a good opportunity to clean up some badly named combinators and fix invalid behaviours.

Since this version introduces a few breaking changes, an [upgrade documentation](https://github.com/Geal/nom/blob/main/doc/upgrading_to_nom_2.md) is available, detailing the steps to fix the most common migration issues. After testing on a set of 30 crates, most of them will build directly, a large part will just need to activate the "verbose-errors" compilation feature. The remaining fixes are documented.

This version also adds a lot of interesting features, like the permutation combinator or whitespace separated formats support.

### Thanks

- @lu-zero for license help
- @adamgreig for type inference fixes
- @keruspe for documentation and example fixes, for the `IResult => Result` conversion work, making `AsChar`'s method more consistent, and adding `many_till!`
- @jdeeny for implementing `Offset` on `&str`
- @vickenty for documentation fixes and his refactoring of `length_value!` and `length_bytes!`
- @overdrivenpotato for refactoring some combinators
- @taralx for documentation fixes
- @keeperofdakeys for fixing eol behaviour, writing documentation and adding `named_attr!`
- @jturner314 for writing documentation
- @bozaro for fixing compilation errors
- @uniphil for adding a `crates.io` badge
- @badboy for documentation fixes
- @jugglerchris for fixing `take_s!`
- @AndyShiue for implementing `Error` and `Display` on `ErrorKind` and detecting incorrect UTF-8 string indexing

### Added

- the "simple" error management system does not accumulates errors when backtracking. This is a big perf gain, and is activated by default in nom 2.0
- nom can now work on any type that implement the traits defined in `src/traits.rs`: `InputLength`, `InputIter`, `InputTake`, `Compare`, `FindToken`, `FindSubstring`, `Slice`
- the documentation from Github's wiki has been moved to the `doc/` directory. They are markdown files that you can build with [cargo-external-doc](https://crates.io/crates/cargo-external-doc)
- whitespace separated format support: with the `ws!` combinator, you can automatically introduce whitespace parsers between all parsers and combinators
- the `permutation!` combinator applies its child parsers in any order, as long as they all succeed once, and return a tuple of the results
- `do_parse!` is a simpler alternative to `chain!`, which is now deprecated
- you can now transform an `IResult` in a `std::result::Result`
- `length_data!` parses a length, and returns a subslice of that length
- `tag_no_case!` provides case independent comparison. It works nicely, without any allocation, for ASCII strings, but for UTF-8 strings, it defaults to an unsatisfying (and incorrect) comparison by lowercasing both strings
- `named_attr!` creates functions like `named!` but can add attributes like documentation
- `many_till!` applies repeatedly its first child parser until the second succeeds

### Changed

- the "verbose" error management that was available in previous versions is now activated by the "verbose-errors" compilation feature
- code reorganization: most of the parsers were moved in separate files to make the source easier to navigate
- most of the combinators are now independent from the input type
- the `eof` function was replaced with the `eof!` macro
- `error!` and `add_error!` were replaced with `return_error!` and `add_return_error!` to fix the name conflict with the log crate
- the `offset()` method is now in the `Offset` trait
- `length_value!` has been renamed to `length_count!`. The new `length_value!` selects a slice and applies the second parser once on that slice
- `AsChar::is_0_to_9` is now `AsChar::is_dec_digit`
- the combinators with configurable endianness now take an enum instead of a boolean as parameter

### Fixed
- the `count!`, `count_fixed!` and `length_*!` combinator calculate incomplete data needs correctly
- `eol`, `line_ending` and `not_line_ending` now have a consistent behaviour that works correctly with incomplete data
- `take_s!` didn't correctly handle the case when the slice is exactly the right length

## 1.2.4 - 2016-07-20

### Thanks
- @Phlosioneer for documentation fixes
- @sourrust for fixing offsets in `take_bits!`
- @ChrisMacNaughton for the XFS crate
- @pwoolcoc for `rest_s`
- @fitzgen for more `IResult` methods
- @gtors for the negative lookahead feature
- @frk1 and @jeandudey for little endian float parsing
- @jethrogb for fixing input usage in `many1`
- @acatton for beating me at nom golf :D

### Added
- the `rest_s` method on `IResult` returns the remaining `&str` input
- `unwrap_err` and `unwrap_inc` methods on `IResult`
- `not!` will peek at the input and return `Done` if the underlying parser returned `Error` or `Incomplete`, without consuming the input
- `le_f32` and `le_f64` parse little endian floating point numbers (IEEE 754)
-

### Fixed
- documentation fixes
- `take_bits!` is now more precise
- `many1` inccorectly used the `len` function instead of `input_len`
- the INI parser is simpler
- `recognize!` had an early `return` that is removed now

## 1.2.3 - 2016-05-10

### Thanks
- @lu-zero for the contribution guidelines
- @GuillaumeGomez for fixes on `length_bytes` and some documentation
- @Hywan for documentation and test fixes
- @Xirdus for correct trait import issues
- @mspiegel for the new AST example
- @cholcombe973 for adding the `cond_with_error!` combinator
- @tstorch for refactoring `many0!`
- @panicbit for the folding combinators
- @evestera for `separated_list!` fixes
- @DanielKeep for correcting some enum imports

### Added
- Regular expression combinators starting with `re_bytes_` work on byte slices
- example parsing arithmetic expressions to an AST
- `cond_with_error!` works like `cond!` but will return `None` if the condition is false, and `Some(value)` if the underlying parser succeeded
- `fold_many0!`, `fold_many1!` and `fold_many_m_n!` will take a parser, an initial value and a combining function, and fold over the successful applications of the parser

### Fixed
- `length_bytes!` converts the result of its child parser to usize
- `take_till!` now imports `InputLength` instead of assuming it's in scope
- `separated_list!` and `separated_nonempty_list!` will not consume the separator if there's no following successfully parsed value
- no more warnings on build

### Changed
- simpler implementation of `many0!`

## 1.2.2 - 2016-03-09

### Thanks
- @conradev for fixing `take_until_s!`
- @GuillaumeGomez for some documentation fixes
- @frewsxcv for some documentation fixes
- @tstorch for some test refactorings

### Added
- `nom::Err` now implements `std::error::Error`

### Fixed
- `hex_u32` does not parses more than 8 chars now
- `take_while!` and `take_while1!` will not perturb the behaviour of `recognize!` anymore

## 1.2.1 - 2016-02-23

### Thanks
- @sourrust for adding methods to `IResult`
- @tstorch for the test refactoring, and for adding methods to `IResult` and `Needed`
- @joelself for fixing the method system

### Added

- mapping methods over `IResult` and `Needed`

### Changed

- `apply_rf` is renamed to `apply_m`. This will not warrant a major version, since it is part missing from the methods feture added in the 1.2.0 release
- the `regexp_macros` feature that used `regex!` to precompile regular expressions has been replaced by the normal regex engine combined with `lazy_static`

### Fixed

- when a parser or combinator was returning an empty buffer as remaining part, it was generating one from a static empty string. This was messing with buffer offset calculation. Now, that empty slice is taken like this: `&input[input.len()..]`.
- The `regexp_macros` and `no_std` feature build again and are now tested with Travis CI

## 1.2.0 - 2016-02-08

### Thanks
- @zentner-kyle for type inference fixes
- @joelself for his work on `&str` parsing and method parsers
- @GuillaumeGomez for implementing methods on `IResult`
- @dirk for the `alt_complete!` combinator
- @tstorch for a lot of refactoring work and unit tests additions
- @jansegre for the hex digit parsers
- @belgum for some documentation fixes
- @lwandrebeck for some documentation fixes and code fixes in `hex_digit`

### Added
- `take_until_and_consume_s!` for consumption of string data until a tag
- more function patterns in `named!`. The error type can now be specified
- `alt_complete!` works like the `alt!` combinator, but tries the next branch if the current one returned `Incomplete`, instead of returning directly
- more unit tests for a lot of combinators
- hexadecimal digit parsers
- the `tuple!` combinator takes a list of parsers as argument, and applies them serially on the input. If all of them are successful, it willr eturn a tuple accumulating all the values. This combinator will (hopefully) replace most uses of `chain!`
- parsers can now be implemented as a method for a struct thanks to the `method!`, `call_m!` and `apply_rf!` combinators

### Fixed
- there were type inference issues in a few combinators. They will now be easier to compile
- `peek!` compilation with bare functions
- `&str` parsers were splitting data at the byte level, not at the char level, which can result in inconsistencies in parsing UTF-8 characters. They now use character indexes
- some method implementations were missing on `IResult<I,O,E>` (with specified error type instead of implicit)

## 1.1.0 - 2016-01-01

This release adds a lot of features related to `&str` parsing. The previous versions
were focused on `&[u8]` and bit streams parsing, but there's a need for more text
parsing with nom. The parsing functions like `alpha`, `digit` and others will now
accept either a `&[u8]` or a `&str`, so there is no breaking change on that part.

There are also a few performance improvements and documentation fixes.

### Thanks
- @Binero for pushing the work on `&str` parsing
- @meh for fixing `Option` and `Vec` imports
- @hoodie for a documentation fix
- @joelself for some documentation fixes
- @vberger for his traits magic making nom functions more generic

### Added

- string related parsers: `tag_s!`, `take_s!`, `is_a_s!`, `is_not_s!`, `take_while_s!`, `take_while1_s!`, `take_till_s!`
- `value!` is a combinator that always returns the same value. If a child parser is passed as second argument, that value is returned when the child parser succeeds

### Changed

- `tag!` will now compare even on partial input. If it expects "abcd" but receives "ef", it will now return an `Error` instead of `Incomplete`
- `many0!` and others will preallocate a larger vector to avoid some copies and reallocations
- `alpha`, `digit`, `alphanumeric`, `space` and `multispace` now accept as input a `&[u8]` or a `&str`. Additionally, they return an error if they receive an empty input
- `take_while!`, `take_while1!`, `take_while_s!`, `take_while1_s!` wilreturn an error on empty input

### Fixed

- if the child parser of `many0!` or `many1!` returns `Incomplete`, it will return `Incomplete` too, possibly updating the needed size
- `Option,` `Some`, `None` and `Vec` are now used with full path imports

## 1.0.1 - 2015-11-22

This releases makes the 1.0 version compatible with Rust 1.2 and 1.3

### Thanks
- @steveklabnik for fixing lifetime issues in Producers and Consumers

## 1.0.0 - 2015-11-16

Stable release for nom. A lot of new features, a few breaking changes

### Thanks
- @ahenry for macro fixes
- @bluss for fixing documentation
- @sourrust for cleaning code and debugging the new streaming utilities
- @meh for inline optimizations
- @ccmtaylor for fixing function imports
- @soro for improvements to the streaming utilities
- @breard-r for catching my typos
- @nelsonjchen for catching my typos too
- @divarvel for hex string parsers
- @mrordinaire for the `length_bytes!` combinator

### Breaking changes
- `IResult::Error` can now use custom error types, and is generic over the input type
- Producers and consumers have been replaced. The new implementation uses less memory and integrates more with parsers
- `nom::ErrorCode` is now `nom::ErrorKind`
- `filter!` has been renamed to `take_while!`
- `chain!` will count how much data is consumed and use that number to calculate how much data is needed if a parser returned `Incomplete`
- `alt!` returns `Incomplete` if a child parser returned `Incomplete`, instead of skipping to the next parser
- `IResult` does not require a lifetime tag anymore, yay!

### Added

- `complete!` will return an error if the child parser returned `Incomplete`
- `add_error!` will wrap an error, but allow backtracking
- `hex_u32` parser

### Fixed
- the behaviour around `Incomplete` is better for most parsers now

## 0.5.0 - 2015-10-16

This release fixes a few issues and stabilizes the code.

### Thanks
- @nox for documentation fixes
- @daboross for linting fixes
- @ahenry for fixing `tap!` and extending `dbg!` and `dbg_dmp!`
- @bluss for tracking down and fixing issues with unsafe code
- @meh for inlining parser functions
- @ccmtaylor for fixing import of `str::from_utf8`

### Fixed
- `tap!`, `dbg!` and `dbg_dmp!` now accept function parameters

### Changed
- the type used in `count_fixed!` must be `Copy`
- `chain!` calculates how much data is needed if one of the parsers returns `Incomplete
- optional parsers in `chain!` can return `Incomplete`

## 0.4.0 - 2015-09-08

Considering the number of changes since the last release, this version can contain breaking changes, so the version number becomes 0.4.0. A lot of new features and performance improvements!

### Thanks
- @frewsxcv for documentation fixes
- @ngrewe for his work on producers and consumers
- @meh for fixes on `chain!` and for the `rest` parser
- @daboross for refactoring `many0!` and `many1!`
- @aleksander for the `switch!` combinator idea
- @TechnoMancer for his help with bit level parsing
- @sxeraverx for pointing out a bug in `is_a!`

### Fixed
- `count_fixed!` must take an explicit type as argument to generate the fixed-size array
- optional parsing behaviour in `chain!`
- `count!` can take 0 elements
- `is_a!` and `is_not!` can now consume the whole input

### Added
- it is now possible to seek to the end of a `MemProducer`
- `opt!` returns `Done(input, None)` if `the child parser returned `Incomplete`
- `rest` will return the remaining input
- consumers can now seek to and from the end of input
- `switch!` applies a first parser then matches on its result to choose the next parser
- bit-level parsers
- character-level parsers
- regular expression parsers
- implementation of `take_till!`, `take_while!` and `take_while1!`

### Changed
- `alt!` can return `Incomplete`
- the error analysis functions will now take references to functions instead of moving them
- performance improvements on producers
- performance improvement for `filter!`
- performance improvement for `count!`: a `Vec` of the right size is directly allocated

## 0.3.11 - 2015-08-04

### Thanks
- @bluss for remarking that the crate included random junk lying non committed in my local repository

### Fixed
- cleanup of my local repository will ship less files in the crates, resulting in a smaller download

## 0.3.10 - 2015-08-03

### Added

- `bits!` for bit level parsing. It indicates that all child parsers will take a `(&[u8], usize)`as input, with the second parameter indicating the bit offset in the first byte. This allows viewing a byte slice as a bit stream. Most combinators can be used directly under `bits!`
- `take_bits!` takes an integer type and a number of bits, consumes that number of bits and updates the offset, possibly by crossing byte boundaries
- bit level parsers are all written in `src/bits.rs`

### Changed

- Parsers that specifically handle bytes have been moved to src/bytes.rs`. This applies to `tag!`, `is_not!`, `is_a!`, `filter!`, `take!`, `take_str!`, `take_until_and_consume!`, `take_until!`, `take_until_either_and_consume!`, `take_until_either!`

## 0.3.9 - 2015-07-20

### Thanks
- @badboy for fixing `filter!`
- @idmit for some documentation fixes

### Added
- `opt_res!` applies a parser and transform its result in a Result. This parser never fails
- `cond_reduce!` takes an expression as parameter, applies the parser if the expression is true, and returns an error if the expression is false
- `tap!` pass the result of a parser to a block to manipulate it, but do not affect the parser's result
- `AccReader` is a Read+BufRead that supports data accumulation and partial consumption. The `consume` method must be called afterwardsto indicate how much was consumed
- Arithmetic expression evaluation and parsing example
- `u16!`, `u32!`, `u64!`, `i16!`, `i32!`, `i64!` take an expression as parameter, if the expression is true, apply the big endian integer parser, if false, the little endian version
- type information for combinators. This will make the documentation a bit easier to navigate

### Fixed
- `map_opt!` and `map_res!` had issues with argument order due to bad macros
- `delimited!` did not compile for certain combinations of arguments
- `filter!` did not return a byte slice but a fixed array

## 0.3.8 - 2015-07-03

### Added
- code coverage is now calculated automatically on Travis CI
- `Stepper`: wrap a `Producer`, and call the method `step` with a parser. This method will buffer data if there is not enough, apply the parser if there is, and keep the rest of the input in memory for the next call
- `ReadProducer`: takes something implementing `Read`, and makes a `Producer` out of it

### Fixed
- the combinators `separated_pair!` and `delimited!` did not work because an implementation macro was not exported
- if a `MemProducer` reached its end, it should always return `Eof`
- `map!` had issues with argument matching

## 0.3.7 - 2015-06-24

### Added
- `expr_res!` and `expr_opt!` evaluate an expression returning a Result or Opt and convert it to IResult
- `AsBytes` is implemented for fixed size arrays. This allows `tag!([41u8, 42u8])`

### Fixed
- `count_fixed!` argument parsing works again

## 0.3.6 - 2015-06-15

### Added
- documentation for a few functions
- the consumer trait now requires the `failed(&self, error_code)` method in case of parsing error
- `named!` now handles the alternative `named!(pub fun_name<OutputType>, ...)`

### Fixed
- `filter!` now returns the whole input if the filter function never returned false
- `take!` casts its argument as usize, so it can accepts any integer type now

## 0.3.5 - 2015-06-10

### Thanks
- @cmr for some documentation fixes

### Added
- `count_fixed!` returns a fixed array

### Fixed
- `count!` is back to the previous behaviour, returning a `Vec` for sizes known at runtime

### Changed
- functions and traits exported from `nom::util` are now directly in `nom::`

## 0.3.4 - 2015-06-09

### Thanks
- @andrew-d for fixes on `cond!`
- @keruspe for features in `chain!`

### Added
- `chain!` can now have mutable fields

### Fixed
- `cond!` had an infinite macro recursion

### Changed
- `chain!` generates less code now. No apprent compilation time improvement

## 0.3.3 - 2015-06-09

### Thanks
- @andrew-d for the little endian signed integer parsers
- @keruspe for fixes on `count!`

### Added
- `le_i8`, `le_i16`, `le_i32`, `le_i64`: little endian signed integer parsers

### Changed
- the `alt!` parser compiles much faster, even with more than 8 branches
- `count!` can now return a fixed size array instead of a growable vector

## 0.3.2 - 2015-05-31

### Thanks
- @keruspe for the `take_str` parser and the function application combinator

### Added
- `take_str!`: takes the specified number of bytes and return a UTF-8 string
- `apply!`: do partial application on the parameters of a function

### Changed
- `Needed::Size` now contains a `usize` instead of a `u32`

## 0.3.1 - 2015-05-21

### Thanks
- @divarvel for the big endian signed integer parsers

### Added
- `be_i8`, `be_i16`, `be_i32`, `be_i64`: big endian signed integer parsers
- the `core` feature can be passed to cargo to build with `no_std`
- colored hexdump can be generated from error chains

## 0.3.0 - 2015-05-07

### Thanks
- @filipegoncalves for some documentation and the new eof parser
- @CrimsonVoid for putting fully qualified types in the macros
- @lu_zero for some documentation fixes

### Added
- new error types that can contain an error code, an input slice, and a list of following errors
- `error!` will cut backtracking and return directly from the parser, with a specified error code
- `eof` parser, successful if there is no more input
- specific error codes for the parsers provided by nom

### Changed
- fully qualified types in macros. A lot of imports are not needed anymore

### Removed
- `FlatMap`, `FlatpMapOpt` and `Functor` traits (replaced by `map!`, `map_opt!` and `map_res!`)

## 0.2.2 - 2015-04-12

### Thanks
- @filipegoncalves and @thehydroimpulse for debugging an infinite loop in many0 and many1
- @thehydroimpulse for suggesting public named parsers
- @skade for removing the dependency on the collections gate

### Added
- `named!` can now declare public functions like this: `named!(pub tst, tag!("abcd"));`
- `pair!(X,Y)` returns a tuple `(x, y)`
- `separated_pair!(X, sep, Y)` returns a tuple `(x, y)`
- `preceded!(opening, X)` returns `x`
- `terminated!(X, closing)` returns `x`
- `delimited(opening, X, closing)` returns `x`
- `separated_list(sep, X)` returns a `Vec<X>`
- `separated_nonempty_list(sep, X)` returns a `Vec<X>` of at list one element

### Changed
- `many0!` and `many1!` forbid parsers that do not consume input
- `is_a!`, `is_not!`, `alpha`, `digit`, `space`, `multispace` will now return an error if they do not consume at least one byte

## 0.2.1 - 2015-04-04

### Thanks
- @mtsr for catching the remaining debug println!
- @jag426 who killed a lot of warnings
- @skade for removing the dependency on the core feature gate


### Added
- little endian unsigned int parsers le_u8, le_u16, le_u32, le_u64
- `count!` to apply a parser a specified number of times
- `cond!` applies a parser if the condition is met
- more parser development tools in `util::*`

### Fixed
- in one case, `opt!` would not compile

### Removed
- most of the feature gates are now removed. The only one still needed is `collections`

## 0.2.0 - 2015-03-24
*works with `rustc 1.0.0-dev (81e2396c7 2015-03-19) (built 2015-03-19)`*

### Thanks
- Ryman for the AsBytes implementation
- jag426 and jaredly for documentation fixes
- eternaleye on #rust IRC for his help on the new macro syntax

### Changed
- the AsBytes trait improves readability, no more b"...", but "..." instead
- Incomplete will now hold either Needed;;Unknown, or Needed::Size(u32). Matching on Incomplete without caring for the value is done with `Incomplete(_)`, but if more granularity is mandatory, `Needed` can be matched too
- `alt!` can pass the result of the parser to a closure
- the `take_*` macros changed behaviour, the default case is now not to consume the separator. The macros have been renamed as follows: `take_until!` -> `take_until_and_consume!`, `take_until_and_leave!` -> `take_until!`, `take_until_either_and_leave!` -> `take_until_either!`, `take_until_either!` -> `take_until_either_and_consume!`

### Added
- `peek!` macro: matches the future input but does not consume it
- `length_value!` macro: the first argument is a parser returning a `n` that can cast to usize, then applies the second parser `n` times. The macro has a variant with a third argument indicating the expected input size for the second parser
- benchmarks are available at https://github.com/Geal/nom_benchmarks
- more documentation
- **Unnamed parser syntax**: warning, this is a breaking change. With this new syntax, the macro combinators do not generate functions anymore, they create blocks. That way, they can be nested, for better readability. The `named!` macro is provided to create functions from parsers. Please be aware that nesting parsers comes with a small cost of compilation time, negligible in most cases, but can quickly get to the minutes scale if not careful. If this happens, separate your parsers in multiple subfunctions.
- `named!`, `closure!` and `call!` macros used to support the unnamed syntax
- `map!`, `map_opt!` and `map_res!` to combine a parser with a normal function, transforming the input directly, or returning an `Option` or `Result`

### Fixed
- `is_a!` is now working properly

### Removed
- the `o!` macro does less than `chain!`, so it has been removed
- the `fold0!` and `fold1!` macros were too complex and awkward to use, the `many*` combinators will be useful for most uses for now

## 0.1.6 - 2015-02-24
### Changed
- consumers must have an end method that will be called after parsing

### Added
- big endian unsigned int and float parsers: be_u8, be_u16, be_u32, be_u64, be_f32, be_f64
- producers can seek
- function and macros documentation
- README documentation
### Fixed
- lifetime declarations
- tag! can return Incomplete

## 0.1.5 - 2015-02-17
### Changed
- traits were renamed: FlatMapper -> FlatMap, Mapper -> FlatMapOpt, Mapper2 -> Functor

### Fixed
- woeks with rustc f1bb6c2f4

## 0.1.4 - 2015-02-17
### Changed
- the chaining macro can take optional arguments with '?'

## 0.1.3 - 2015-02-16
### Changed
- the chaining macro now takes the closure at the end of the argument list

## 0.1.2 - 2015-02-16
### Added
- flat_map implementation for <&[u8], &[u8]>
- chaining macro
- partial MP4 parser example


## 0.1.1 - 2015-02-06
### Fixed
- closure syntax change

## Compare code

* [unreleased](https://github.com/Geal/nom/compare/7.1.3...HEAD)
* [7.1.2](https://github.com/Geal/nom/compare/7.1.2...7.1.3)
* [7.1.2](https://github.com/Geal/nom/compare/7.1.1...7.1.2)
* [7.1.1](https://github.com/Geal/nom/compare/7.1.0...7.1.1)
* [7.1.0](https://github.com/Geal/nom/compare/7.0.0...7.1.0)
* [7.0.0](https://github.com/Geal/nom/compare/6.2.1...7.0.0)
* [6.2.1](https://github.com/Geal/nom/compare/6.2.0...6.2.1)
* [6.2.0](https://github.com/Geal/nom/compare/6.1.2...6.2.0)
* [6.1.2](https://github.com/Geal/nom/compare/6.1.1...6.1.2)
* [6.1.1](https://github.com/Geal/nom/compare/6.1.0...6.1.1)
* [6.1.0](https://github.com/Geal/nom/compare/6.0.1...6.1.0)
* [6.0.1](https://github.com/Geal/nom/compare/6.0.0...6.0.1)
* [6.0.0](https://github.com/Geal/nom/compare/5.1.1...6.0.0)
* [5.1.1](https://github.com/Geal/nom/compare/5.1.0...5.1.1)
* [5.1.0](https://github.com/Geal/nom/compare/5.0.1...5.1.0)
* [5.0.1](https://github.com/Geal/nom/compare/5.0.0...5.0.1)
* [5.0.0](https://github.com/Geal/nom/compare/4.2.3...5.0.0)
* [4.2.3](https://github.com/Geal/nom/compare/4.2.2...4.2.3)
* [4.2.2](https://github.com/Geal/nom/compare/4.2.1...4.2.2)
* [4.2.1](https://github.com/Geal/nom/compare/4.2.0...4.2.1)
* [4.2.0](https://github.com/Geal/nom/compare/4.1.1...4.2.0)
* [4.1.1](https://github.com/Geal/nom/compare/4.1.0...4.1.1)
* [4.1.0](https://github.com/Geal/nom/compare/4.0.0...4.1.0)
* [4.0.0](https://github.com/Geal/nom/compare/3.2.1...4.0.0)
* [3.2.1](https://github.com/Geal/nom/compare/3.2.0...3.2.1)
* [3.2.0](https://github.com/Geal/nom/compare/3.1.0...3.2.0)
* [3.1.0](https://github.com/Geal/nom/compare/3.0.0...3.1.0)
* [3.0.0](https://github.com/Geal/nom/compare/2.2.1...3.0.0)
* [2.2.1](https://github.com/Geal/nom/compare/2.2.0...2.2.1)
* [2.2.0](https://github.com/Geal/nom/compare/2.1.0...2.2.0)
* [2.1.0](https://github.com/Geal/nom/compare/2.0.1...2.1.0)
* [2.0.1](https://github.com/Geal/nom/compare/2.0.0...2.0.1)
* [2.0.0](https://github.com/Geal/nom/compare/1.2.4...2.0.0)
* [1.2.4](https://github.com/Geal/nom/compare/1.2.3...1.2.4)
* [1.2.3](https://github.com/Geal/nom/compare/1.2.2...1.2.3)
* [1.2.2](https://github.com/Geal/nom/compare/1.2.1...1.2.2)
* [1.2.1](https://github.com/Geal/nom/compare/1.2.0...1.2.1)
* [1.2.0](https://github.com/Geal/nom/compare/1.1.0...1.2.0)
* [1.1.0](https://github.com/Geal/nom/compare/1.0.1...1.1.0)
* [1.0.1](https://github.com/Geal/nom/compare/1.0.0...1.0.1)
* [1.0.0](https://github.com/Geal/nom/compare/0.5.0...1.0.0)
* [0.5.0](https://github.com/geal/nom/compare/0.4.0...0.5.0)
* [0.4.0](https://github.com/geal/nom/compare/0.3.11...0.4.0)
* [0.3.11](https://github.com/geal/nom/compare/0.3.10...0.3.11)
* [0.3.10](https://github.com/geal/nom/compare/0.3.9...0.3.10)
* [0.3.9](https://github.com/geal/nom/compare/0.3.8...0.3.9)
* [0.3.8](https://github.com/Geal/nom/compare/0.3.7...0.3.8)
* [0.3.7](https://github.com/Geal/nom/compare/0.3.6...0.3.7)
* [0.3.6](https://github.com/Geal/nom/compare/0.3.5...0.3.6)
* [0.3.5](https://github.com/Geal/nom/compare/0.3.4...0.3.5)
* [0.3.4](https://github.com/Geal/nom/compare/0.3.3...0.3.4)
* [0.3.3](https://github.com/Geal/nom/compare/0.3.2...0.3.3)
* [0.3.2](https://github.com/Geal/nom/compare/0.3.1...0.3.2)
* [0.3.1](https://github.com/Geal/nom/compare/0.3.0...0.3.1)
* [0.3.0](https://github.com/Geal/nom/compare/0.2.2...0.3.0)
* [0.2.2](https://github.com/Geal/nom/compare/0.2.1...0.2.2)
* [0.2.1](https://github.com/Geal/nom/compare/0.2.0...0.2.1)
* [0.2.0](https://github.com/Geal/nom/compare/0.1.6...0.2.0)
* [0.1.6](https://github.com/Geal/nom/compare/0.1.5...0.1.6)
* [0.1.5](https://github.com/Geal/nom/compare/0.1.4...0.1.5)
* [0.1.4](https://github.com/Geal/nom/compare/0.1.3...0.1.4)
* [0.1.3](https://github.com/Geal/nom/compare/0.1.2...0.1.3)
* [0.1.2](https://github.com/Geal/nom/compare/0.1.1...0.1.2)
* [0.1.1](https://github.com/Geal/nom/compare/0.1.0...0.1.1)
