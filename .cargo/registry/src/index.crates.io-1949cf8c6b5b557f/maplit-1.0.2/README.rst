maplit
======

Container / collection literal macros for `HashMap <https://doc.rust-lang.org/beta/std/collections/struct.HashMap.html>`_, `HashSet <https://doc.rust-lang.org/beta/std/collections/struct.HashSet.html>`_, `BTreeMap <https://doc.rust-lang.org/beta/std/collections/struct.BTreeMap.html>`_, `BTreeSet <https://doc.rust-lang.org/beta/std/collections/struct.BTreeSet.html>`_.

You can use these for convenience. Using them has no other implications.

Please read the `API documentation here`__

__ https://docs.rs/maplit/

|build_status|_ |crates|_

.. |build_status| image:: https://travis-ci.org/bluss/maplit.svg?branch=master
.. _build_status: https://travis-ci.org/bluss/maplit

.. |crates| image:: http://meritbadge.herokuapp.com/maplit
.. _crates: https://crates.io/crates/maplit


Recent Changes
--------------

- 1.0.2

  - Fix usage of the macros through full paths, like `maplit::hashmap!(..)` (#27)

- 1.0.1

  - Fix ``unused_results`` lint in the macros by @povilasb

- 1.0.0

  - maplit 1.0!
  - Only documentation changes since the last version

- 0.1.6

  - Add macro ``convert_args!`` for composable opt-in conversion of the
    expressions being used for the maplit macros.

- 0.1.5

  - Add license files correctly
  - Add crates.io category
  - Small doc improvements by @seeekr and @sanmai-NL

- 0.1.4

  - Update docs to point to docs.rs

- 0.1.2

  - Now supports more arguments in hashset!{} and hashmap!{}

- 0.1.0

  - Initial release

FAQ
---

**Question:** Very large maps take a long time to compile?

**Answer:** Rustc is very slow to compile big expressions with many literals
(including integers and float literals). Work around this by either
using explicitly typed literals, or explicitly typed conversions.
See https://github.com/bluss/maplit/issues/14 for more information.


License
-------

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
http://www.apache.org/licenses/LICENSE-2.0 or the MIT license
http://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.
