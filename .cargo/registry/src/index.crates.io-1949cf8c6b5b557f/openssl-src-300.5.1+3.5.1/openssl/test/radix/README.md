RADIX Test Framework
====================

Purpose
-------

This directory contains the RADIX test framework, which is a six-dimension
script-driven facility intended to facilitate execution of

- multi-stream
- multi-client
- multi-server
- multi-thread
- multi-process (in future)
- multi-node (in future)

test vignettes for network protocol testing applications. While it is currently
used for QUIC, it has been designed to be agnostic so that it can be adapted to
other protocols in future if desired.

In particular, unlike the older multistream test framework, it does not assume a
single client and a single server. Examples of vignettes designed to be
supported by the RADIX test framework in future include:

- single client ↔ single server
- multiple clients ↔ single server
- single client ↔ multiple servers
- multiple clients ↔ multiple servers

“Multi-process” and “multi-node” means there has been some consideration
given to support of multi-process and multi-node testing in the future, though
this is not currently supported.

Differences to `quic_multistream_test`
--------------------------------------

The RADIX test features the following improvements relative to the
`quic_multistream_test` framework:

- Due to usage of the new QUIC server API, opcodes are no longer duplicated
  into “C” and “S” variants. There is symmetry between all endpoints in this
  regard. The legacy `QUIC_TSERVER` facility is not used and will eventually
  be retired once other test suites no longer rely on it.

- The framework is not limited to two objects (client and server),
  but can instead instantiate an arbitrary number of SSL objects and pass these
  to operations. Other kinds of object could be supported in future if needed.

- Scripts are now generated dynamically at launch time by functions rather
  than hardcoding them into the executable. This has the advantage that scripts
  can be generated dynamically. In particular, this allows very long
  procedurally generated test scripts which would be impractical to write
  by hand. This can be used for stress testing, for example.

- As a result of the fact that scripts are now generated dynamically, the
  in-memory representation of script operations has been improved as the ability
  to write a script operation as an initializer list is no longer required. The
  new representation is simpler, more compact, and more flexible. A stack-based
  approach allows arbitrary argument types to be passed as operands to an
  operation, rather than having a fixed number of operands of fixed type for all
  script operations.

- Scripts are now named, rather than numbered, giving more useful debug output,
  and hopefully reducing the frequency of merge conflicts.

- Debug logging has in general been significantly improved and has been designed
  to be accessible and useful.

- Logging of child threads is now buffered and printed after a test is complete,
  which avoids non-deterministic interleaving of test output.

- The number of core opcodes for the interpreter has been dramatically reduced
  and now the vast majority of test operations are performed via `OP_FUNC`,
  which is similar to `OP_CHECK`. This change is largely transparent to the
  test developer.

- In the future, multi-process or multi-node testing will be supported.
  The expectation is that multi-node testing will be facilitated by having the
  master process invoke a hook shell script which is responsible for propping up
  and tearing down the additional nodes. Writing a suitable hook script will be
  left as an exercise to the test infrastructure maintainer. This abstracts the
  test framework from the details of a specific infrastructure environment and
  its management tools (VMs, containers, etc.).

- The core test interpreter is designed to be agnostic to QUIC and could be
  used for testing other protocols in the future. There is a clean, layered
  design which draws a clear distinction between the core interpreter,
  protocol-specific bindings and support code, test operation definitions, and
  script definitions.

- It is no longer needed to explicitly set the ALPN using an opcode when
  establishing a connection using QUIC. Since ALPN is required for QUIC, the
  first opcode of every test was used to set the ALPN to the same string, which
  was redundant. This is now done automatically.

- An explicit `OP_END` is no longer needed.

Architecture
------------

The RADIX test suite framework is built in four layers:

- **TERP** ([terp.c](./terp.c)), a protocol-agnostic stack-based script
  interpreter for interruptible execution of test vignettes;

- the **QUIC bindings** ([quic_bindings.c](./quic_bindings.c)), which defines
  QUIC-specific test framework;

- the **QUIC operations** ([quic_ops.c](./quic_ops.c)), which define specific
  test operations for TERP which can be invoked by QUIC unit tests on top of the
  basic infrastructure defined by the QUIC bindings;

- the QUIC unit tests ([quic_tests.c](./quic_tests.c)), which use the above
  QUIC bindings.
