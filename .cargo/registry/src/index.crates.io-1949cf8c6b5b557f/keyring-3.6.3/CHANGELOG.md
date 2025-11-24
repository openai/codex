## Version 3.6.2
- Have docs.rs build docs for all modules on all platforms (thanks to @unkcpz - see #235).
- Switch to `fastrand` for tests (see #237).

## Version 3.6.1
- Updated dependencies; no code changes.

## Version 3.6.0
- Add combination keystore of keyutils and secret service (thanks to @soywod).

## Version 3.5.0
- Add debug logging of internal operations (thanks to @soywod).
- Revert iOS security-framework dependency to v2 (see #225).

## Version 3.4.0
- Allow use of both secret-service and keyutils.

## Version 3.3.0
- Add support for credential-store attributes other than those used by this crate.  This allows the creation of credentials that are more compatible with 3rd-party clients, such as the OS-provided GUIs over credentials.
- Make the textual descriptions of entries consistently follow the form `user@service` (or  `user@service:target` if a target was specified).

## Version 3.2.1
- Re-enable access to v1 credentials. The fixes of version 3.2 meant that legacy credentials with no target attribute couldn't be accessed.

## Version 3.2.0
- Improve secret-service handling of targets, so that searches on locked items distinguish items with different targets properly.

## Version 3.1.0
- enhance the CLI to allow empty user names and better info about `Ambiguous` credentials.

## Version 3.0.5
- updated docs and clean up dead code. No code changes.

## Version 3.0.4
- expose a cross-platform module alias via the `default` module.

## Version 3.0.3
- fix feature `linux-native`, which was causing compile errors.

## Version 3.0.2
- add missing implementations for iOS `set_secret` and `get_secret`

## Version 3.0.1
- add back missing `Sync` trait on errors.

## Version 3.0.0
- add `dbus-secret-service` dependency to allow use on \*n\*x without an async runtime
- (API change) rework feature controls on included keystores: now there is a feature for each keystore, and that keystore is included in a build if and only if its feature is specified *and* the keystore is supported by the target OS.
- (API change) add direct support for setting and reading binary secret data, not just UTF-8 strings.

## Version 2.0.1
- fix the example in the README.

## Version 2.0
- (API change) Allow creation of entries to fail.
- (API change) Introduce an ambiguous error on credential lookup.
- (API change) Make the `Error` enum non-exhaustive.
- (API change) Introduce traits for pluggable credential-store implementations.  (This removes the old `platform` module.)
- Add a `mock` credential store for easy cross-platform client testing.
- Upgrade to secret-service v3.
- Always use service-level search in secret-service.
- Allow creation of new collections in secret-service.
- Add the kernel keyutils as a linux credential store.
- Add build support for FreeBSD (thanks @ryanavella).

## Version 1.2.1
- password length was not validated correctly on Windows (#85)

## Version 1.2
- introduce protection against the use of empty arguments

## Version 1.1.2
- replace `structopt` with new, improved `clap` that incorporates all the same functionality.

## Version 1.1.1
- no functional updates, just documentation improvements

## Version 1.1.0
- add iOS support

## Version 1.0.1
- fix #80: missing winapi features (jyuch)

## Version 1.0.0
- Breaking API changes:
  - `Keyring` struct renamed to `Entry`
  - `KeyringError` enum renamed to `Error`, and is completely cross-platform.
- API enhancements:
  - Clients can now control how entries map to credentials; see `Entry::new_with_target` and `Entry::new_with_credential`
  - Clients can now retrieve platform credentials with metadata rather than just passwords; see `Entry::get_password_and_credential`.
  - Non UTF8 passwords now have their data available.
  - Non-login keychains are usable on Linux and Mac.
- Expanded documentation and `cli` example.

## Version 0.10.4
- CI fix for linux executable

## Version 0.10.3
- Added NoPassword and NoBackend errors to windows code (phillip couto)
- Update dependencies: (brotskydotcom)
    - secret-service from 1.1.1 to 2.0.2
    - security-framework from 0.4.2 to 2.4.2
- Update CI/tests, readme (brotskydotcom)

## Version 0.10.2
- yanked, release snafu

## Version 0.10.1
- update to secret-service 1.1.1

## Version 0.10.0
- ability to access named keychains in macos (nagasunilt)

## Version 0.9.0
- upgrade security-framework 0.3.0 -> 0.4.2
- upgrade secret-service 1.0.0 -> 1.1.0 (updates hkdf dep, fixes error handling related to missing collection)

## Version 0.8.0
- Upgrade to winapi 0.3 and removes advapi32-sys from windows.
- Upgrades to edition 2018
- Formats everything to 1.40
- Removes mem::uninitialized from windows.

## Version 0.7.1
- only include application name on create password, not on get password

## Version 0.7.0
- cli binary moved to examples.
- osx now uses `security-framework` library instead of cli.
- hex dependency removed on osx.
- update to `secret-service` for linux, which
  - removes gmp as a dependency
  - updates rust-crypto to RustCrypto
  - correctly encrypts/decrypts blank input
- tests moved to `lib.rs`

Plan to move to 1.0 if this version is stable.

## Version 0.6.1
- bug fix for special characters on osx.

## Version 0.6.0
- fix behavior in windows where third-party editing of password would result in malformed retrieved password. The solution was to convert all strings to and from Windows utf16, where before I was passing the secret as a blob from utf8.
- remove dependency on rustcserialize, use hex.
- update rpassword to 2.0, removing dependency on termios
- fix some mistakes in syntax for targeting dependencies to an os.

## Version 0.5.1
- remove some unwraps which were causing a problem in linux

## Version 0.5
- bumped secret-service to 0.4.0, which improved error-handling around emptyr passwords a bit more (in 0.3.1), and made gmp dependency optional in 0.4.0

## Version 0.4
- yanked. But originally was trying to handle secret-service empty password better. But there was an error in secret-service 0.3

## Version 0.3
- Windows support!

## Version 0.2

- Fix major bug in decoding output on osx. Now handles both regular and "special" (non-ascii) utf8 chars appropriately.
- add simple tests for the fix!
- add changelog.

## Version 0.1

- linux implementation using secret-service backend.
- osx implementation using security cli.
