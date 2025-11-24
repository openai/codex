/*!

# Linux (keyutils) store with Secret Service backing

This store, contributed by [@soywod](https://github.com/soywod),
uses the [keyutils module](crate::keyutils) as a cache
available to headless processes, while using the
[secret-service module](crate::secret_service)
to provide credential storage beyond reboot.
The expected usage pattern
for this module is as follows:

- Processes that run on headless systems are built with `keyutils` support via the
  `linux-native` feature of this crate. After each reboot, these processes
  are either launched after the keyutils cache has been reloaded from the secret service,
  or (if launched immediately) they wait until the keyutils cache has been reloaded.
- A headed "configuration" process is built with this module that allows its user
  to configure the credentials needed by the headless processes. After each reboot,
  this process unlocks the secret service (see both the keyutils and secret-service
  module for information about how this can be done headlessly, if desired) and then
  accesses each of the configured credentials (which loads them into keyutils). At
  that point the headless clients can be started (or become active, if already started).

This store works by creating a keyutils entry and a secret-service entry for
each of its entries. Because keyutils entries don't have attributes, entries
in this store don't expose attributes either. Because keyutils entries can't
store empty passwords/secrets, this store's entries can't either.

See the documentation for the `keyutils` and `secret-service` modules if you
want details about how the underlying storage is handled.
 */

use log::debug;

use super::credential::{
    Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi, CredentialPersistence,
};
use super::error::{Error, Result};
use super::keyutils::KeyutilsCredential;
use super::secret_service::{SsCredential, SsCredentialBuilder};

/// Representation of a keyutils-persistent credential.
///
/// The credential owns a [KeyutilsCredential] for in-memory usage and
/// a [SsCredential] for persistence.
#[derive(Debug, Clone)]
pub struct KeyutilsPersistentCredential {
    keyutils: KeyutilsCredential,
    ss: SsCredential,
}

impl CredentialApi for KeyutilsPersistentCredential {
    /// Set a password in the underlying store
    fn set_password(&self, password: &str) -> Result<()> {
        self.set_secret(password.as_bytes())
    }

    /// Set a secret in the underlying store
    ///
    /// It sets first the secret in keyutils, then in
    /// secret-service. If the latter set fails, the former
    /// is reverted.
    fn set_secret(&self, secret: &[u8]) -> Result<()> {
        let prev_secret = self.keyutils.get_secret();
        self.keyutils.set_secret(secret)?;

        if let Err(err) = self.ss.set_secret(secret) {
            debug!("Failed set of secret-service: {err}; reverting keyutils");
            match prev_secret {
                Ok(ref secret) => self.keyutils.set_secret(secret),
                Err(Error::NoEntry) => self.keyutils.delete_credential(),
                Err(err) => Err(err),
            }?;

            return Err(err);
        }

        Ok(())
    }

    /// Retrieve a password from the underlying store
    ///
    /// The password is retrieved from keyutils. In case of error, the
    /// password is retrieved from secret-service instead (and
    /// keyutils is updated).
    fn get_password(&self) -> Result<String> {
        match self.keyutils.get_password() {
            Ok(password) => {
                return Ok(password);
            }
            Err(err) => {
                debug!("Failed get from keyutils: {err}; trying secret service")
            }
        }

        let password = self.ss.get_password().map_err(ambiguous_to_no_entry)?;
        self.keyutils.set_password(&password)?;

        Ok(password)
    }

    /// Retrieve a secret from the underlying store
    ///
    /// The secret is retrieved from keyutils. In case of error, the
    /// secret is retrieved from secret-service instead (and keyutils
    /// is updated).
    fn get_secret(&self) -> Result<Vec<u8>> {
        match self.keyutils.get_secret() {
            Ok(secret) => {
                return Ok(secret);
            }
            Err(err) => {
                debug!("Failed get from keyutils: {err}; trying secret service")
            }
        }

        let secret = self.ss.get_secret().map_err(ambiguous_to_no_entry)?;
        self.keyutils.set_secret(&secret)?;

        Ok(secret)
    }

    /// Delete a password from the underlying store.
    ///
    /// The credential is deleted from both keyutils and
    /// secret-service.
    fn delete_credential(&self) -> Result<()> {
        if let Err(err) = self.keyutils.delete_credential() {
            debug!("cannot delete keyutils credential: {err}");
        }

        self.ss.delete_credential()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn debug_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl KeyutilsPersistentCredential {
    /// Create the platform credential for a Keyutils entry.
    ///
    /// This just passes the arguments to the underlying two stores
    /// and wraps their results with an entry that holds both.
    pub fn new_with_target(target: Option<&str>, service: &str, user: &str) -> Result<Self> {
        let ss = SsCredential::new_with_target(target, service, user)?;
        let keyutils = KeyutilsCredential::new_with_target(target, service, user)?;
        Ok(Self { keyutils, ss })
    }
}

/// The builder for keyutils-persistent credentials
#[derive(Debug, Default)]
pub struct KeyutilsPersistentCredentialBuilder {}

/// Returns an instance of the keyutils-persistent credential builder.
///
/// If keyutils-persistent is the default credential store, this is
/// called once when an entry is first created.
pub fn default_credential_builder() -> Box<CredentialBuilder> {
    Box::new(KeyutilsPersistentCredentialBuilder {})
}

impl CredentialBuilderApi for KeyutilsPersistentCredentialBuilder {
    /// Build a [KeyutilsPersistentCredential] for the given target, service, and user.
    fn build(&self, target: Option<&str>, service: &str, user: &str) -> Result<Box<Credential>> {
        Ok(Box::new(SsCredential::new_with_target(
            target, service, user,
        )?))
    }

    /// Return the underlying builder object with an `Any` type so that it can
    /// be downgraded to a [KeyutilsPersistentCredentialBuilder] for platform-specific processing.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// Return the persistence of this store.
    ///
    /// This store's persistence derives from that of the secret service.
    fn persistence(&self) -> CredentialPersistence {
        SsCredentialBuilder {}.persistence()
    }
}

/// Replace any Ambiguous error with a NoEntry one
fn ambiguous_to_no_entry(err: Error) -> Error {
    if let Error::Ambiguous(_) = err {
        return Error::NoEntry;
    };

    err
}

#[cfg(test)]
mod tests {
    use crate::{Entry, Error};

    use super::KeyutilsPersistentCredential;

    fn entry_new(service: &str, user: &str) -> Entry {
        crate::tests::entry_from_constructor(
            KeyutilsPersistentCredential::new_with_target,
            service,
            user,
        )
    }

    #[test]
    fn test_invalid_parameter() {
        let credential = KeyutilsPersistentCredential::new_with_target(Some(""), "service", "user");
        assert!(
            matches!(credential, Err(Error::Invalid(_, _))),
            "Created entry with empty target"
        );
    }

    #[test]
    fn test_empty_service_and_user() {
        crate::tests::test_empty_service_and_user(entry_new);
    }

    #[test]
    fn test_missing_entry() {
        crate::tests::test_missing_entry(entry_new);
    }

    #[test]
    fn test_empty_password() {
        let entry = entry_new("empty password service", "empty password user");
        assert!(
            matches!(entry.set_password(""), Err(Error::Invalid(_, _))),
            "Able to set empty password"
        );
    }

    #[test]
    fn test_round_trip_ascii_password() {
        crate::tests::test_round_trip_ascii_password(entry_new);
    }

    #[test]
    fn test_round_trip_non_ascii_password() {
        crate::tests::test_round_trip_non_ascii_password(entry_new);
    }

    #[test]
    fn test_round_trip_random_secret() {
        crate::tests::test_round_trip_random_secret(entry_new);
    }

    #[test]
    fn test_update() {
        crate::tests::test_update(entry_new);
    }

    #[test]
    fn test_noop_get_update_attributes() {
        crate::tests::test_noop_get_update_attributes(entry_new);
    }
}
