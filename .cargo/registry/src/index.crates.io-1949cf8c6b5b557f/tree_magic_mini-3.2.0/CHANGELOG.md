# tree_magic_mini 3.0.0

* Split GPL-licensed files into a separate optional dependency. The main crate
  is now MIT-licensed, and searches for data files installed on the system at
  run-time by default.

  If you enable the `with-gpl-data` feature, then the data files will be
  hard-coded into the library at compile time.  Programs that use this feature
  must be distributed according to the terms of the GNU GPL 2.0 or later.

# tree_magic_mini 2.0.0

* Change license to GPL-2.0-or-later for compatibility with upstream
  xdg-shared-mime-info license.

# tree_magic_mini 1.0.1

* Update to nom 6.

# tree_magic_mini 1.0.0

* Forked and changed name to `tree_magic_mini`
* Updated dependencies.
* Reduced copying and memory allocation, for a slight increase in speed and
  decrease in memory use.
* Reduced API surface. Some previously public APIs are now internal.
* Removed the optional `cli` feature and `tmagic` binary.

# 0.2.3

Upgraded package versions to latest (except nom, which is currently stuck at
3.x) and fixed the paths in the doc tests 

# 0.2.2

Yanked due to accidental breaking API change

# 0.2.1

Incorporated fix by Bram Sanders to prevent panic on non-existent file.

# 0.2.0

Major changes, front-end and back.

- Added `is_alias` function
- `from_*` functions excluding `from_*_node` now return MIME, not Option<MIME>
- New feature flag: `staticmime`. Changes type of MIME from String to &'static str
- Bundled magic file, so it works on Windows as well.
- Split `fdo_magic` checker into `fdo_magic::sys` and `fdo_magic::builtin`
- `len` argument removed from `*_u8` functions
- Tests and benchmarks added.
- Fixed horribly broken logic in `fdo_magic` checker
- Checks the most common types before obscure types
- Changed hasher to `fnv`.
- Added support for handling aliases in input
- `tmagic` command has more features
- Major speed improvements

# 0.1.1
 
- *Changed public interface*: Added `from_u8` export function
- *Changed public interface*: Changed len argument for `u8` functions from `u32` to `usize`
- Minor speed improvements in `fdo_magic` checker
 
# 0.1.0
 
Initial release
