#![cfg_attr(docsrs, feature(doc_cfg))]
/*!

# Keyring

This is a cross-platform library that does storage and retrieval of passwords
(or other secrets) in an underlying platform-specific secure store.
A top-level introduction to the library's usage, as well as a small code sample,
may be found in [the library's entry on crates.io](https://crates.io/crates/keyring).
Currently supported platforms are
Linux,
FreeBSD,
OpenBSD,
Windows,
macOS, and iOS.

## Design

This crate implements a very simple, platform-independent concrete object called an _entry_.
Each entry is identified by a <_service name_, _user name_> pair of UTF-8 strings,
optionally augmented by a _target_ string (which can be used to distinguish two entries
that have the same _service name_ and _user name_).
Entries support setting, getting, and forgetting (aka deleting) passwords (UTF-8 strings)
and binary secrets (byte arrays).

Entries provide persistence for their passwords by wrapping credentials held in platform-specific
credential stores.  The implementations of these platform-specific stores are captured
in two types (with associated traits):

- a _credential builder_, represented by the [CredentialBuilder] type
  (and [CredentialBuilderApi](credential::CredentialBuilderApi) trait).  Credential
  builders are given the identifying information provided for an entry and map
  it to the identifying information for a platform-specific credential.
- a _credential_, represented by the [Credential] type
  (and [CredentialApi](credential::CredentialApi) trait).  The platform-specific credential
  identified by a builder for an entry is what provides the secure storage
  for that entry's password/secret.

## Crate-provided Credential Stores

This crate runs on several different platforms, and it provides one
or more implementations of credential stores on each platform.
These implementations work by mapping the data used to identify an entry
to data used to identify platform-specific storage objects.
For example, on macOS, the service and user provided for an entry
are mapped to the service and user attributes that identify a
generic credential in the macOS keychain.

Typically, platform-specific stores (called _keystores_ in this crate)
have a richer model of a credential than
the one used by this crate to identify entries.
These keystores expose their specific model in the
concrete credential objects they use to implement the Credential trait.
In order to allow clients to access this richer model, the Credential trait
has an [as_any](credential::CredentialApi::as_any) method that returns a
reference to the underlying
concrete object typed as [Any](std::any::Any), so that it can be downgraded to
its concrete type.

### Credential store features

Each of the platform-specific credential stores is associated with one or more features.
These features control whether that store is included when the crate is built.  For
example, the macOS Keychain credential store is only included if the `"apple-native"`
feature is specified (and the crate is built with a macOS target).

If no specified credential store features apply to a given platform,
this crate will use the (platform-independent) _mock_ credential store (see below)
on that platform. There are no
default features in this crate: you must specify explicitly which platform-specific
credential stores you intend to use.

Here are the available credential store features:

- `apple-native`: Provides access to the Keychain credential store on macOS and iOS.

- `windows-native`: Provides access to the Windows Credential Store on Windows.

- `linux-native`: Provides access to the `keyutils` storage on Linux.

- `linux-native-sync-persistent`: Uses both `keyutils` and `sync-secret-service`
  (see below) for storage. See the docs for the `keyutils_persistent`
  module for a full explanation of why both are used. Because this
  store uses the `sync-secret-service`, you can use additional features related
  to that store (described below).

- `linux-native-async-persistent`: Uses both `keyutils` and `async-secret-service`
  (see below) for storage. See the docs for the `keyutils_persistent`
  module for a full explanation of why both are used.
  Because this store uses the `async-secret-service`, you
  must specify the additional features required by that store (described below).

- `sync-secret-service`: Provides access to the DBus-based
  [Secret Service](https://specifications.freedesktop.org/secret-service/latest/)
  storage on Linux, FreeBSD, and OpenBSD.  This is a _synchronous_ keystore that provides
  support for encrypting secrets when they are transferred across the bus. If you wish
  to use this encryption support, additionally specify one (and only one) of the
  `crypto-rust` or `crypto-openssl` features (to choose the implementation libraries
  used for the encryption). By default, this keystore requires that the DBus library be
  installed on the user's machine (and the openSSL library if you specify it for
  encryption), but you can avoid this requirement by specifying the `vendored` feature
  (which will cause the build to include those libraries statically).

- `async-secret-service`: Provides access to the DBus-based
  [Secret Service](https://specifications.freedesktop.org/secret-service/latest/)
  storage on Linux, FreeBSD, and OpenBSD.  This is an _asynchronous_ keystore that
  always encrypts secrets when they are transferred across the bus. You _must_ specify
  both an async runtime feature (either `tokio` or `async-io`) and a cryptographic
  implementation (either `crypto-rust` or `crypto-openssl`) when using this
  keystore. If you want to use openSSL encryption but those libraries are not
  installed on the user's machine, specify the `vendored` feature
  to statically link them with the built crate.

The Linux platform is the only one for which this crate supplies multiple keystores:
native (keyutils), sync or async secret service, and sync or async "combo" (both
keyutils and secret service). You cannot specify use of both sync and async
keystores; this will lead to a compile error.  If you enable a combo keystore on Linux,
that will be the default keystore. If you don't enable a
combo keystore on Linux, but you do enable both the native and secret service keystores,
the secret service will be the default.

## Client-provided Credential Stores

In addition to the platform stores implemented by this crate, clients
are free to provide their own secure stores and use those.  There are
two mechanisms provided for this:

- Clients can give their desired credential builder to the crate
  for use by the [Entry::new] and [Entry::new_with_target] calls.
  This is done by making a call to [set_default_credential_builder].
  The major advantage of this approach is that client code remains
  independent of the credential builder being used.

- Clients can construct their concrete credentials directly and
  then turn them into entries by using the [Entry::new_with_credential]
  call. The major advantage of this approach is that credentials
  can be identified however clients want, rather than being restricted
  to the simple model used by this crate.

## Mock Credential Store

In addition to the platform-specific credential stores, this crate
always provides a mock credential store that clients can use to
test their code in a platform independent way.  The mock credential
store allows for pre-setting errors as well as password values to
be returned from [Entry] method calls.

## Interoperability with Third Parties

Each of the platform-specific credential stores provided by this crate uses
an underlying store that may also be used by modules written
in other languages.  If you want to interoperate with these third party
credential writers, then you will need to understand the details of how the
target, service, and user of this crate's generic model
are used to identify credentials in the platform-specific store.
These details are in the implementation of this crate's secure-storage
modules, and are documented in the headers of those modules.

(_N.B._ Since the included credential store implementations are platform-specific,
you may need to use the Platform drop-down on [docs.rs](https://docs.rs/keyring) to
view the storage module documentation for your desired platform.)

## Caveats

This module expects passwords to be UTF-8 encoded strings,
so if a third party has stored an arbitrary byte string
then retrieving that as a password will return a
[BadEncoding](Error::BadEncoding) error.
The returned error will have the raw bytes attached,
so you can access them, but you can also just fetch
them directly using [get_secret](Entry::get_secret) rather than
[get_password](Entry::get_password).

While this crate's code is thread-safe, the underlying credential
stores may not handle access from different threads reliably.
In particular, accessing the same credential
from multiple threads at the same time can fail, especially on
Windows and Linux, because the accesses may not be serialized in the same order
they are made. And for RPC-based credential stores such as the dbus-based Secret
Service, accesses from multiple threads (and even the same thread very quickly)
are not recommended, as they may cause the RPC mechanism to fail.
 */

use log::debug;
use std::collections::HashMap;

pub use credential::{Credential, CredentialBuilder};
pub use error::{Error, Result};

pub mod mock;

//
// can't use both sync and async secret service
//
#[cfg(any(
    all(feature = "sync-secret-service", feature = "async-secret-service"),
    all(
        feature = "linux-native-sync-persistent",
        feature = "linux-native-async-persistent",
    )
))]
compile_error!("This crate cannot use both the sync and async versions of any credential store");

//
// pick the *nix keystore
//
#[cfg(all(target_os = "linux", feature = "linux-native"))]
#[cfg_attr(docsrs, doc(cfg(target_os = "linux")))]
pub mod keyutils;
#[cfg(all(
    target_os = "linux",
    feature = "linux-native",
    not(feature = "sync-secret-service"),
    not(feature = "async-secret-service"),
))]
pub use keyutils as default;

#[cfg(all(
    any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"),
    any(feature = "sync-secret-service", feature = "async-secret-service"),
))]
#[cfg_attr(
    docsrs,
    doc(cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd")))
)]
pub mod secret_service;
#[cfg(all(
    any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"),
    any(feature = "sync-secret-service", feature = "async-secret-service"),
    not(any(
        feature = "linux-native-sync-persistent",
        feature = "linux-native-async-persistent",
    )),
))]
pub use secret_service as default;

#[cfg(all(
    target_os = "linux",
    any(
        feature = "linux-native-sync-persistent",
        feature = "linux-native-async-persistent",
    )
))]
#[cfg_attr(docsrs, doc(cfg(target_os = "linux")))]
pub mod keyutils_persistent;
#[cfg(all(
    target_os = "linux",
    any(
        feature = "linux-native-sync-persistent",
        feature = "linux-native-async-persistent",
    ),
))]
pub use keyutils_persistent as default;

// fallback to mock if neither keyutils nor secret service is available
#[cfg(any(
    all(
        target_os = "linux",
        not(feature = "linux-native"),
        not(feature = "sync-secret-service"),
        not(feature = "async-secret-service"),
    ),
    all(
        any(target_os = "freebsd", target_os = "openbsd"),
        not(feature = "sync-secret-service"),
        not(feature = "async-secret-service"),
    )
))]
pub use mock as default;

//
// pick the Apple keystore
//
#[cfg(all(target_os = "macos", feature = "apple-native"))]
#[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
pub mod macos;
#[cfg(all(target_os = "macos", feature = "apple-native"))]
pub use macos as default;
#[cfg(all(target_os = "macos", not(feature = "apple-native")))]
pub use mock as default;

#[cfg(all(target_os = "ios", feature = "apple-native"))]
#[cfg_attr(docsrs, doc(cfg(target_os = "ios")))]
pub mod ios;
#[cfg(all(target_os = "ios", feature = "apple-native"))]
pub use ios as default;
#[cfg(all(target_os = "ios", not(feature = "apple-native")))]
pub use mock as default;

//
// pick the Windows keystore
//
#[cfg(all(target_os = "windows", feature = "windows-native"))]
#[cfg_attr(docsrs, doc(cfg(target_os = "windows")))]
pub mod windows;
#[cfg(all(target_os = "windows", not(feature = "windows-native")))]
pub use mock as default;
#[cfg(all(target_os = "windows", feature = "windows-native"))]
pub use windows as default;

#[cfg(not(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "macos",
    target_os = "ios",
    target_os = "windows",
)))]
pub use mock as default;

pub mod credential;
pub mod error;

#[derive(Default, Debug)]
struct EntryBuilder {
    inner: Option<Box<CredentialBuilder>>,
}

static DEFAULT_BUILDER: std::sync::RwLock<EntryBuilder> =
    std::sync::RwLock::new(EntryBuilder { inner: None });

/// Set the credential builder used by default to create entries.
///
/// This is really meant for use by clients who bring their own credential
/// store and want to use it everywhere.  If you are using multiple credential
/// stores and want precise control over which credential is in which store,
/// then use [new_with_credential](Entry::new_with_credential).
///
/// This will block waiting for all other threads currently creating entries
/// to complete what they are doing. It's really meant to be called
/// at app startup before you start creating entries.
pub fn set_default_credential_builder(new: Box<CredentialBuilder>) {
    let mut guard = DEFAULT_BUILDER
        .write()
        .expect("Poisoned RwLock in keyring-rs: please report a bug!");
    guard.inner = Some(new);
}

fn build_default_credential(target: Option<&str>, service: &str, user: &str) -> Result<Entry> {
    static DEFAULT: std::sync::OnceLock<Box<CredentialBuilder>> = std::sync::OnceLock::new();
    let guard = DEFAULT_BUILDER
        .read()
        .expect("Poisoned RwLock in keyring-rs: please report a bug!");
    let builder = guard
        .inner
        .as_ref()
        .unwrap_or_else(|| DEFAULT.get_or_init(|| default::default_credential_builder()));
    let credential = builder.build(target, service, user)?;
    Ok(Entry { inner: credential })
}

#[derive(Debug)]
pub struct Entry {
    inner: Box<Credential>,
}

impl Entry {
    /// Create an entry for the given service and user.
    ///
    /// The default credential builder is used.
    ///
    /// # Errors
    ///
    /// This function will return an [Error] if the `service` or `user` values are invalid.
    /// The specific reasons for invalidity are platform-dependent, but include length constraints.
    ///
    /// # Panics
    ///
    /// In the very unlikely event that the internal credential builder's `RwLock`` is poisoned, this function
    /// will panic. If you encounter this, and especially if you can reproduce it, please report a bug with the
    /// details (and preferably a backtrace) so the developers can investigate.
    pub fn new(service: &str, user: &str) -> Result<Entry> {
        debug!("creating entry with service {service}, user {user}, and no target");
        let entry = build_default_credential(None, service, user)?;
        debug!("created entry {:?}", entry.inner);
        Ok(entry)
    }

    /// Create an entry for the given target, service, and user.
    ///
    /// The default credential builder is used.
    pub fn new_with_target(target: &str, service: &str, user: &str) -> Result<Entry> {
        debug!("creating entry with service {service}, user {user}, and target {target}");
        let entry = build_default_credential(Some(target), service, user)?;
        debug!("created entry {:?}", entry.inner);
        Ok(entry)
    }

    /// Create an entry that uses the given platform credential for storage.
    pub fn new_with_credential(credential: Box<Credential>) -> Entry {
        debug!("create entry from {credential:?}");
        Entry { inner: credential }
    }

    /// Set the password for this entry.
    ///
    /// Can return an [Ambiguous](Error::Ambiguous) error
    /// if there is more than one platform credential
    /// that matches this entry.  This can only happen
    /// on some platforms, and then only if a third-party
    /// application wrote the ambiguous credential.
    pub fn set_password(&self, password: &str) -> Result<()> {
        debug!("set password for entry {:?}", self.inner);
        self.inner.set_password(password)
    }

    /// Set the secret for this entry.
    ///
    /// Can return an [Ambiguous](Error::Ambiguous) error
    /// if there is more than one platform credential
    /// that matches this entry.  This can only happen
    /// on some platforms, and then only if a third-party
    /// application wrote the ambiguous credential.
    pub fn set_secret(&self, secret: &[u8]) -> Result<()> {
        debug!("set secret for entry {:?}", self.inner);
        self.inner.set_secret(secret)
    }

    /// Retrieve the password saved for this entry.
    ///
    /// Returns a [NoEntry](Error::NoEntry) error if there isn't one.
    ///
    /// Can return an [Ambiguous](Error::Ambiguous) error
    /// if there is more than one platform credential
    /// that matches this entry.  This can only happen
    /// on some platforms, and then only if a third-party
    /// application wrote the ambiguous credential.
    pub fn get_password(&self) -> Result<String> {
        debug!("get password from entry {:?}", self.inner);
        self.inner.get_password()
    }

    /// Retrieve the secret saved for this entry.
    ///
    /// Returns a [NoEntry](Error::NoEntry) error if there isn't one.
    ///
    /// Can return an [Ambiguous](Error::Ambiguous) error
    /// if there is more than one platform credential
    /// that matches this entry.  This can only happen
    /// on some platforms, and then only if a third-party
    /// application wrote the ambiguous credential.
    pub fn get_secret(&self) -> Result<Vec<u8>> {
        debug!("get secret from entry {:?}", self.inner);
        self.inner.get_secret()
    }

    /// Get the attributes on the underlying credential for this entry.
    ///
    /// Some of the underlying credential stores allow credentials to have named attributes
    /// that can be set to string values. See the documentation for each credential store
    /// for a list of which attribute names are supported by that store.
    ///
    /// Returns a [NoEntry](Error::NoEntry) error if there isn't a credential for this entry.
    ///
    /// Can return an [Ambiguous](Error::Ambiguous) error
    /// if there is more than one platform credential
    /// that matches this entry.  This can only happen
    /// on some platforms, and then only if a third-party
    /// application wrote the ambiguous credential.
    pub fn get_attributes(&self) -> Result<HashMap<String, String>> {
        debug!("get attributes from entry {:?}", self.inner);
        self.inner.get_attributes()
    }

    /// Update the attributes on the underlying credential for this entry.
    ///
    /// Some of the underlying credential stores allow credentials to have named attributes
    /// that can be set to string values. See the documentation for each credential store
    /// for a list of which attribute names can be given values by this call. To support
    /// cross-platform use, each credential store ignores (without error) any specified attributes
    /// that aren't supported by that store.
    ///
    /// Returns a [NoEntry](Error::NoEntry) error if there isn't a credential for this entry.
    ///
    /// Can return an [Ambiguous](Error::Ambiguous) error
    /// if there is more than one platform credential
    /// that matches this entry.  This can only happen
    /// on some platforms, and then only if a third-party
    /// application wrote the ambiguous credential.
    pub fn update_attributes(&self, attributes: &HashMap<&str, &str>) -> Result<()> {
        debug!(
            "update attributes for entry {:?} from map {attributes:?}",
            self.inner
        );
        self.inner.update_attributes(attributes)
    }

    /// Delete the underlying credential for this entry.
    ///
    /// Returns a [NoEntry](Error::NoEntry) error if there isn't one.
    ///
    /// Can return an [Ambiguous](Error::Ambiguous) error
    /// if there is more than one platform credential
    /// that matches this entry.  This can only happen
    /// on some platforms, and then only if a third-party
    /// application wrote the ambiguous credential.
    ///
    /// Note: This does _not_ affect the lifetime of the [Entry]
    /// structure, which is controlled by Rust.  It only
    /// affects the underlying credential store.
    pub fn delete_credential(&self) -> Result<()> {
        debug!("delete entry {:?}", self.inner);
        self.inner.delete_credential()
    }

    /// Return a reference to this entry's wrapped credential.
    ///
    /// The reference is of the [Any](std::any::Any) type, so it can be
    /// downgraded to a concrete credential object.  The client must know
    /// what type of concrete object to cast to.
    pub fn get_credential(&self) -> &dyn std::any::Any {
        self.inner.as_any()
    }
}

#[cfg(doctest)]
doc_comment::doctest!("../README.md", readme);

#[cfg(test)]
/// There are no actual tests in this module.
/// Instead, it contains generics that each keystore invokes in their tests,
/// passing their store-specific parameters for the generic ones.
//
// Since iOS doesn't use any of these generics, we allow dead code.
#[allow(dead_code)]
mod tests {
    use super::{credential::CredentialApi, Entry, Error, Result};
    use std::collections::HashMap;

    /// Create a platform-specific credential given the constructor, service, and user
    pub fn entry_from_constructor<F, T>(f: F, service: &str, user: &str) -> Entry
    where
        F: FnOnce(Option<&str>, &str, &str) -> Result<T>,
        T: 'static + CredentialApi + Send + Sync,
    {
        match f(None, service, user) {
            Ok(credential) => Entry::new_with_credential(Box::new(credential)),
            Err(err) => {
                panic!("Couldn't create entry (service: {service}, user: {user}): {err:?}")
            }
        }
    }

    /// Create a platform-specific credential given the constructor, service, user, and attributes
    pub fn entry_from_constructor_and_attributes<F, T>(
        f: F,
        service: &str,
        user: &str,
        attrs: &HashMap<&str, &str>,
    ) -> Entry
    where
        F: FnOnce(Option<&str>, &str, &str, &HashMap<&str, &str>) -> Result<T>,
        T: 'static + CredentialApi + Send + Sync,
    {
        match f(None, service, user, attrs) {
            Ok(credential) => Entry::new_with_credential(Box::new(credential)),
            Err(err) => {
                panic!("Couldn't create entry (service: {service}, user: {user}): {err:?}")
            }
        }
    }

    /// A basic round-trip unit test given an entry and a password.
    pub fn test_round_trip(case: &str, entry: &Entry, in_pass: &str) {
        entry
            .set_password(in_pass)
            .unwrap_or_else(|err| panic!("Can't set password for {case}: {err:?}"));
        let out_pass = entry
            .get_password()
            .unwrap_or_else(|err| panic!("Can't get password for {case}: {err:?}"));
        assert_eq!(
            in_pass, out_pass,
            "Passwords don't match for {case}: set='{in_pass}', get='{out_pass}'",
        );
        entry
            .delete_credential()
            .unwrap_or_else(|err| panic!("Can't delete password for {case}: {err:?}"));
        let password = entry.get_password();
        assert!(
            matches!(password, Err(Error::NoEntry)),
            "Read deleted password for {case}",
        );
    }

    /// A basic round-trip unit test given an entry and a password.
    pub fn test_round_trip_secret(case: &str, entry: &Entry, in_secret: &[u8]) {
        entry
            .set_secret(in_secret)
            .unwrap_or_else(|err| panic!("Can't set secret for {case}: {err:?}"));
        let out_secret = entry
            .get_secret()
            .unwrap_or_else(|err| panic!("Can't get secret for {case}: {err:?}"));
        assert_eq!(
            in_secret, &out_secret,
            "Passwords don't match for {case}: set='{in_secret:?}', get='{out_secret:?}'",
        );
        entry
            .delete_credential()
            .unwrap_or_else(|err| panic!("Can't delete password for {case}: {err:?}"));
        let password = entry.get_secret();
        assert!(
            matches!(password, Err(Error::NoEntry)),
            "Read deleted password for {case}",
        );
    }

    /// When tests fail, they leave keys behind, and those keys
    /// have to be cleaned up before the tests can be run again
    /// in order to avoid bad results.  So it's a lot easier just
    /// to have tests use a random string for key names to avoid
    /// the conflicts, and then do any needed cleanup once everything
    /// is working correctly.  So we export this function for tests to use.
    pub fn generate_random_string_of_len(len: usize) -> String {
        use fastrand;
        use std::iter::repeat_with;
        repeat_with(fastrand::alphanumeric).take(len).collect()
    }

    pub fn generate_random_string() -> String {
        generate_random_string_of_len(30)
    }

    fn generate_random_bytes_of_len(len: usize) -> Vec<u8> {
        use fastrand;
        use std::iter::repeat_with;
        repeat_with(|| fastrand::u8(..)).take(len).collect()
    }

    pub fn test_empty_service_and_user<F>(f: F)
    where
        F: Fn(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let in_pass = "doesn't matter";
        test_round_trip("empty user", &f(&name, ""), in_pass);
        test_round_trip("empty service", &f("", &name), in_pass);
        test_round_trip("empty service & user", &f("", ""), in_pass);
    }

    pub fn test_missing_entry<F>(f: F)
    where
        F: FnOnce(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let entry = f(&name, &name);
        assert!(
            matches!(entry.get_password(), Err(Error::NoEntry)),
            "Missing entry has password"
        )
    }

    pub fn test_empty_password<F>(f: F)
    where
        F: FnOnce(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let entry = f(&name, &name);
        test_round_trip("empty password", &entry, "");
    }

    pub fn test_round_trip_ascii_password<F>(f: F)
    where
        F: FnOnce(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let entry = f(&name, &name);
        test_round_trip("ascii password", &entry, "test ascii password");
    }

    pub fn test_round_trip_non_ascii_password<F>(f: F)
    where
        F: FnOnce(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let entry = f(&name, &name);
        test_round_trip("non-ascii password", &entry, "このきれいな花は桜です");
    }

    pub fn test_round_trip_random_secret<F>(f: F)
    where
        F: FnOnce(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let entry = f(&name, &name);
        let secret = generate_random_bytes_of_len(24);
        test_round_trip_secret("non-ascii password", &entry, secret.as_slice());
    }

    pub fn test_update<F>(f: F)
    where
        F: FnOnce(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let entry = f(&name, &name);
        test_round_trip("initial ascii password", &entry, "test ascii password");
        test_round_trip(
            "updated non-ascii password",
            &entry,
            "このきれいな花は桜です",
        );
    }

    pub fn test_noop_get_update_attributes<F>(f: F)
    where
        F: FnOnce(&str, &str) -> Entry,
    {
        let name = generate_random_string();
        let entry = f(&name, &name);
        assert!(
            matches!(entry.get_attributes(), Err(Error::NoEntry)),
            "Read missing credential in attribute test",
        );
        let mut map: HashMap<&str, &str> = HashMap::new();
        map.insert("test attribute name", "test attribute value");
        assert!(
            matches!(entry.update_attributes(&map), Err(Error::NoEntry)),
            "Updated missing credential in attribute test",
        );
        // create the credential and test again
        entry
            .set_password("test password for attributes")
            .unwrap_or_else(|err| panic!("Can't set password for attribute test: {err:?}"));
        match entry.get_attributes() {
            Err(err) => panic!("Couldn't get attributes: {err:?}"),
            Ok(attrs) if attrs.is_empty() => {}
            Ok(attrs) => panic!("Unexpected attributes: {attrs:?}"),
        }
        assert!(
            matches!(entry.update_attributes(&map), Ok(())),
            "Couldn't update attributes in attribute test",
        );
        match entry.get_attributes() {
            Err(err) => panic!("Couldn't get attributes after update: {err:?}"),
            Ok(attrs) if attrs.is_empty() => {}
            Ok(attrs) => panic!("Unexpected attributes after update: {attrs:?}"),
        }
        entry
            .delete_credential()
            .unwrap_or_else(|err| panic!("Can't delete credential for attribute test: {err:?}"));
        assert!(
            matches!(entry.get_attributes(), Err(Error::NoEntry)),
            "Read deleted credential in attribute test",
        );
    }
}
