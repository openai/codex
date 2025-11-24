# tree_magic_mini

`tree_magic_mini` is a Rust crate that determines the MIME type a given file
or byte stream. 

Read the documentation at https://docs.rs/tree_magic_mini/

This is a fork of the [tree_magic](https://crates.io/crates/tree_magic)
crate by Allison Hancock. It includes the following changes:

* Updated dependencies.
* Reduced copying and memory allocation, for a slight increase in speed and
  decrease in memory use.
* Reduced API surface. Some previously public APIs are now internal.
* Removed the optional `cli` feature and `tmagic` binary.
* Split GPL-licensed data files into a separate optional crate.

These changes were made both to make the library more efficient, and to
simplify the effort to maintain and optimize of this fork. I would like to
eventually merge these changes back to the original `tree_magic` crate, and/or
restore some of the removed features if there is demand for that.

## Licensing and the MIME database

By default, `tree_magic_mini` will attempt to load the shared MIME info
database from the standard locations at runtime.

If you won't have the database files available, or would like to include them
in your binary for simplicity, you can optionally embed the database
information if you enable the `tree_magic_db` feature.

**As the magic database files themselves are licensed under the GPL, you must
make sure your project uses a compatible license if you enable this behaviour.**

---

Continue reading for the original `tree_magic` documentation.

## About tree_magic

Unlike the typical approach that libmagic and file(1) uses, this loads all the file types in a tree based on subclasses. (EX: `application/vnd.openxmlformats-officedocument.wordprocessingml.document` (MS Office 2007) subclasses `application/zip` which subclasses `application/octet-stream`) Then, instead of checking the file against *every* file type, it can traverse down the tree and only check the file types that make sense to check. (After all, the fastest check is the check that never gets run.)

This library also provides the ability to check if a file is a certain type without going through the process of checking it against every file type.

## Performance

This is fast. FAST.

This is a test of my Downloads folder (sorry, can't find a good publicly available set of random files) on OpenSUSE Tumbleweed. `tmagic` was compiled with `cargo build --release`, and `file` came from the OpenSUSE repos. This is a warm run, which means I've ran both programs through a few times. System is a dual-core Intel Core i7 640M, and results were measured with `time`.

Program | real | user | sys
--------|------|------|-----
tmagic 0.2.0 | 0m0.063s | 0m0.052s | 0m0.004s
file-5.30 --mime-type | 0m0.924s | 0.800s | 0.116s

There's a couple things that lead to this. Mainly:

- Less types to parse due to graph approach.

- First 4K of file is loaded then passed to all parsers, instead of constantly reloading from disk. (When doing that, the time was more around ~0.130s.)

- The most common types (image/png, image/jpeg, application/zip, etc.) are checked before the exotic ones.

- Everything that can be lazily initialized is.

Nightly users can also run `cargo bench` for some benchmarks. For tree_magic 0.2.0 on the same hardware:

    test from_u8::application_zip  ... bench:      17,086 ns/iter (+/- 845)
    test from_u8::image_gif        ... bench:       5,027 ns/iter (+/- 520)
    test from_u8::image_png        ... bench:       4,421 ns/iter (+/- 1,795)
    test from_u8::text_plain       ... bench:     112,578 ns/iter (+/- 11,778)
    test match_u8::application_zip ... bench:         222 ns/iter (+/- 144)
    test match_u8::image_gif       ... bench:         140 ns/iter (+/- 14)
    test match_u8::image_png       ... bench:         139 ns/iter (+/- 18)
    test match_u8::text_plain      ... bench:          44 ns/iter (+/- 3)

However, it should be noted that the FreeDesktop.org magic files less filetypes than the magic files used by libmagic. (On my system tree_magic supports 400 types, while `/usr/share/misc/magic` contains 855 `!:mime` tags.) It is, however, significantly easier to parse, as it only covers magic numbers and not attributes or anything like that. See the TODO section for plans to fix this.

## Compatibility

This has been tested using Rust Stable and Nightly on Windows 7 and OpenSUSE Tumbleweed Linux.

All mime information and relation information is loaded from the Shared MIME-info Database as described at https://specifications.freedesktop.org/shared-mime-info-spec/shared-mime-info-spec-latest.html. If you beleive that this is not present on your system, turn off the `sys_fdo_magic` feature flag.

This provides the most common file types, but it's still missing some important ones, like LibreOffice or MS Office 2007+ support or ISO files. Expect this to improve, especially as the `zip` checker is added.

### Architecture

`tree_magic` is split up into different "checker" modules. Each checker handles a certain set of filetypes, and only those. For instance, the `basetype` checker handles the `inode/*` and `text/plain` types, while the `fdo_magic` checker handles anything with a magic number. Th idea here is that instead of following the `libmagic` route of having one magic descriptor format that fits every file, we can specialize and choose the checker that suits the file format best.

During library initialization, each checker is queried for the types is supports and the parent->child relations between them. During this time, the checkers can load any rules, schemas, etc. into memory. A big philosophy here is that **time during the checking phase is many times more valuable than during the init phase**. The library only gets initialized once, and the library can check thousands of files during a program's lifetime.

From the list of file types and relations, a directed graph is built, and each node is added to a hash map. The library user can use these directly to find parents, children, etc. of a given MIME if needed.

When a file needs to be checked against a certain MIME (match_*), each checker is queried to see if it supports that type, and if so, it runs the checker. If the checker returns true, it must be that type.

When a file needs it's MIME type found (from_*), the library starts at the `all/all` node of the type graph (or whichever node the user specifies) and walks down the tree. If a match is found, it continues searching down that branch. If no match is found, it retrieves the deepest MIME type found.

## TODO

### Improve fdo-magic checker

Right now the `fdo-magic` checker does not handle endianess. It also does not handle magic files stored in the user's home directory.

### Additional checkers

It is planned to have custom file checking functions for many types. Here's some ideas:

- `zip`: Everything that subclasses `application/zip` can be determined further by peeking at the zip's directory listing. 

- `grep`: Text files such as program scripts and configuration files could be parsed with a regex (or whatever works best). 

- `json`, `toml`, `xml`, etc: Check the given file against a schema and return true if it matches. (By this point there should be few enough potential matches that it should be okay to load the entire file)

- (specialized parsers): Binary (or text) files without any sort of magic can be checked for compliance against a quick and dirty `nom` parser instead of the weird heuristics used by libmagic.

To add additional checker types, add a new module exporting:

- `init::get_supported() -> Vec<(String)>`

- `init::get_subclasses() -> Vec<String, String)>`

- `test::from_u8(&[u8], &str) -> bool`

- `test::from_filpath(&str, &str) -> Result<bool, std::io::Error>`
    
and then add references to those functions into the CHECKERS lazy_static! in `lib.rs`. The bottommost entries get searched first.

### Caching

Going forward, it is essential for a checker (like `basetype`'s metadata, or that json/toml/xml example) to be able to cache an in-memory representation of the file, so it doesn't have to get re-loaded and re-parsed for every new type. With the current architecture, this is rather difficult to implement.

### Multiple file types

There are some weird files out there ( [Polyglot quines](https://en.wikipedia.org/wiki/Polyglot_(computing)) come to mind. ) that are multiple file types. This might be worth handling for security reasons. (It's not a huge priority, though.)

### Parallel processing

Right now this is single-threaded. This is an embarasingly parallel task (multiple files, multiple types, multiple rules for each type...), so there should be a great speed benefit.

## TO NOT DO

### File attributes

`libmagic` and `file`, by default, print descriptive strings detailing the file type and, for things like JPEG images or ELF files, a whole bunch of metadata. This is not something `tree_magic` will ever support, as it is entirely unnecessary. Support for attributes would best be handled in a seperate crate that, given a MIME, can extract metadata in a predictable, machine readable format.
