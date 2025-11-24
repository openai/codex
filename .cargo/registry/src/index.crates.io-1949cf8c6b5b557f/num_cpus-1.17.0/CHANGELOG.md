## v1.17.0

### Fixes

- update hermit-abi to 0.5.0
- remove special support for nacl

## v1.16.0

### Features

- add support for AIX operating system

### Fixes

- update hermit-abi to 0.3.0

## v1.15.0

### Fixes

- update hermit-abi

## v1.14.0

### Features

- add support for cgroups v2
- Skip reading files in Miri

## v1.13.1

### Fixes

- fix parsing zero or multiple optional fields in cgroup mountinfo.

## v1.13.0

### Features

- add Linux cgroups support when calling `get()`.

## v1.12.0

#### Fixes

- fix `get` on OpenBSD to ignore offline CPUs
- implement `get_physical` on OpenBSD

## v1.11.1

#### Fixes

- Use `mem::zeroed` instead of `mem::uninitialized`.

## v1.11.0

#### Features

- add `hermit` target OS support
- removes `bitrig` support

#### Fixes

- fix `get_physical` count with AMD hyperthreading.

## v1.10.1

#### Fixes

- improve `haiku` CPU detection

## v1.10.0

#### Features

- add `illumos` target OS support
- add default fallback if target is unknown to `1`

## v1.9.0

#### Features

- add `sgx` target env support

## v1.8.0

#### Features

- add `wasm-unknown-unknown` target support

## v1.7.0

#### Features

- add `get_physical` support for macOS

#### Fixes

- use `_SC_NPROCESSORS_CONF` on Unix targets

### v1.6.2

#### Fixes

- revert 1.6.1 for now

### v1.6.1

#### Fixes

- fixes sometimes incorrect num on Android/ARM Linux (#45)

## v1.6.0

#### Features

- `get_physical` gains Windows support

### v1.5.1

#### Fixes

- fix `get` to return 1 if `sysconf(_SC_NPROCESSORS_ONLN)` failed

## v1.5.0

#### Features

- `get()` now checks `sched_affinity` on Linux

## v1.4.0

#### Features

- add `haiku` target support

## v1.3.0

#### Features

- add `redox` target support

### v1.2.1

#### Fixes

- fixes `get_physical` count (454ff1b)

## v1.2.0

#### Features

- add `emscripten` target support
- add `fuchsia` target support

## v1.1.0

#### Features

- added `get_physical` function to return number of physical CPUs found

# v1.0.0

#### Features

- `get` function returns number of CPUs (physical and virtual) of current platform
