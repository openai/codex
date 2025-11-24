/*!

# Linux kernel (keyutils) credential store

Modern linux kernels have a built-in secure store,
[keyutils](https://www.man7.org/linux/man-pages/man7/keyutils.7.html).
This module (written primarily by [@landhb](https://github.com/landhb))
uses that secure store as the persistent back end for entries.

Entries in keyutils are identified by a string `description`.  If an entry is created with
an explicit `target`, that value is used as the keyutils description.  Otherwise, the string
`keyring-rs:user@service` is used (where user and service come from the entry creation call).

There is no notion of attribute other than the description supported by keyutils,
so the [get_attributes](crate::Entry::get_attributes)
and [update_attributes](crate::Entry::update_attributes)
calls are both no-ops for this credential store.

# Persistence

The key management facility provided by the kernel is completely in-memory and will not persist
across reboots. Consider the keyring a secure cache and plan for your application to handle
cases where the entry is no-longer available in-memory.

In general you should prepare for `Entry::get_password` to fail and have a fallback to re-load
the credential into memory.

Potential options to re-load the credential into memory are:

- Re-prompt the user (most common/effective for CLI applications)
- Create a PAM module or use `pam_exec` to load a credential securely when the user logs in.
- If you're running as a systemd service you can use `systemd-ask-password` to prompt the user
  when your service starts.

```
use std::error::Error;
use keyring::Entry;

/// Simple user code that handles retrieving a credential regardless
/// of the credential state.
struct CredentialManager {
    entry: Entry,
}

impl CredentialManager {
    /// Init the service as normal
    pub fn new(service: &str, user: &str) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            entry: Entry::new(service, user)?
        })
    }

    /// Method that first attempts to retreive the credential from memory
    /// and falls back to prompting the user.
    pub fn get(&self) -> Result<String, Box<dyn Error>> {
        self.entry.get_password().or_else(|_| self.prompt())
    }

    /// Internal method to prompt the user and cache the credential
    /// in memory for subsequent lookups.
    fn prompt(&self) -> Result<String, Box<dyn Error>> {
        let password = rpassword::read_password()?;
        self.entry.set_password(&password)?;
        Ok(password)
    }
}
```

A single entry in keyutils can be on multiple "keyrings", each of which has a subtly
different lifetime.  The core storage for keyring keys is provided by the user-specific
[persistent keyring](https://www.man7.org/linux/man-pages/man7/persistent-keyring.7.html),
whose lifetime defaults to a few days (and is controllable by
administrators).  But whenever an entry's credential is used,
it is also added to the user's
[session keyring](https://www.man7.org/linux/man-pages/man7/session-keyring.7.html):
this ensures that the credential will persist as long as the user session exists, and when the user
logs out the credential will persist as long as the persistent keyring doesn't expire while the user is
logged out.

Each time the `Entry::new()` operation is performed, the persistent keyring's expiration timer
is reset to the value configured in:

```no_run,no_test,ignore
proc/sys/kernel/keys/persistent_keyring_expiry
```

| Persistent Keyring State | Session Keyring State | User Key State |
| -------------            | -------------         | -------------  |
| Active                   | Active                | Active         |
| Expired                  | Active                | Active         |
| Active                   | Logged Out            | Active (Accessible on next login)        |
| Expired                  | Logged Out            | Expired        |

**Note**: As mentioned above, a reboot clears all keyrings.

## Headless usage

If you are trying to use keyring on a headless linux box, it's strongly recommended that you use this
credential store, because (as part of the kernel) it's designed to be used headlessly.
To set this module as your default store, build with `--features linux-default-keyutils`.
Alternatively, you can drop the secret-service credential store altogether
(which will slim your build significantly) by building keyring
with `--no-default-features` and `--features linux-no-secret-service`.

 */

use super::credential::{
    Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi, CredentialPersistence,
};
use super::error::{decode_password, Error as ErrorCode, Result};
use linux_keyutils::{KeyError, KeyRing, KeyRingIdentifier};

/// Representation of a keyutils credential.
///
/// Since the CredentialBuilderApi::build method does not provide
/// an initial secret, and it is impossible to have 0-length keys,
/// this representation holds a linux_keyutils::KeyRing instead
/// of a linux_keyutils::Key.
///
/// The added benefit of this approach
/// is that any call to get_password before set_password is done
/// will result in a proper error as the key does not exist until
/// set_password is called.
#[derive(Debug, Clone)]
pub struct KeyutilsCredential {
    /// Host session keyring
    pub session: KeyRing,
    /// Host persistent keyring
    pub persistent: Option<KeyRing>,
    /// Description of the key entry
    pub description: String,
}

impl CredentialApi for KeyutilsCredential {
    /// Set a password in the underlying store
    ///
    /// This will overwrite the entry if it already exists since
    /// it's using `add_key` under the hood.
    ///
    /// Returns an [Invalid](ErrorCode::Invalid) error if the password
    /// is empty, because keyutils keys cannot have empty values.
    fn set_password(&self, password: &str) -> Result<()> {
        self.set_secret(password.as_bytes())
    }

    fn set_secret(&self, secret: &[u8]) -> Result<()> {
        if secret.is_empty() {
            return Err(ErrorCode::Invalid(
                "secret".to_string(),
                "cannot be empty".to_string(),
            ));
        }

        // Add to the session keyring
        let key = self
            .session
            .add_key(&self.description, secret)
            .map_err(decode_error)?;

        // Directly link to the persistent keyring as well
        if let Some(keyring) = self.persistent {
            keyring.link_key(key).map_err(decode_error)?;
        }
        Ok(())
    }

    /// Retrieve a password from the underlying store
    ///
    /// This requires a call to `Key::read` with checked conversions
    /// to a utf8 Rust string.
    fn get_password(&self) -> Result<String> {
        let secret = self.get_secret()?;
        // Attempt utf-8 conversion
        decode_password(secret)
    }

    /// Retrieve a secret from the underlying store
    ///
    /// This requires a call to `Key::read`.
    fn get_secret(&self) -> Result<Vec<u8>> {
        // Verify that the key exists and is valid
        let key = self
            .session
            .search(&self.description)
            .map_err(decode_error)?;

        // Directly re-link to the session keyring
        // If a logout occurred, it will only be linked to the
        // persistent keyring, and needs to be added again.
        self.session.link_key(key).map_err(decode_error)?;

        // Directly re-link to the persistent keyring
        // If it expired, it will only be linked to the
        // session keyring, and needs to be added again.
        if let Some(keyring) = self.persistent {
            keyring.link_key(key).map_err(decode_error)?;
        }

        // Read in the key (making sure we have enough room)
        let buffer = key.read_to_vec().map_err(decode_error)?;
        Ok(buffer)
    }

    /// Delete a password from the underlying store.
    ///
    /// Under the hood this uses `Key::invalidate` to immediately
    /// invalidate the key and prevent any further successful
    /// searches.
    ///
    /// Note that the keyutils implementation uses caching,
    /// and the caches take some time to clear,
    /// so a key that has been invalidated may still be found
    /// by get_password if it's called within milliseconds
    /// in *the same process* that deleted the key.
    fn delete_credential(&self) -> Result<()> {
        // Verify that the key exists and is valid
        let key = self
            .session
            .search(&self.description)
            .map_err(decode_error)?;

        // Invalidate the key immediately
        key.invalidate().map_err(decode_error)?;
        Ok(())
    }

    /// Cast the credential object to std::any::Any.  This allows clients
    /// to downcast the credential to its concrete type so they
    /// can do platform-specific things with it.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// Expose the concrete debug formatter for use via the [Credential] trait
    fn debug_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl KeyutilsCredential {
    /// Create a credential from the matching keyutils key.
    ///
    /// This is basically a no-op, because keys don't have extra attributes,
    /// but at least we make sure the underlying platform credential exists.
    pub fn get_credential(&self) -> Result<Self> {
        self.session
            .search(&self.description)
            .map_err(decode_error)?;
        Ok(self.clone())
    }

    /// Create the platform credential for a Keyutils entry.
    ///
    /// An explicit target string is interpreted as the KeyRing to use for the entry.
    /// If none is provided, then we concatenate the user and service in the string
    /// `keyring-rs:user@service`.
    pub fn new_with_target(target: Option<&str>, service: &str, user: &str) -> Result<Self> {
        // Obtain the session keyring
        let session =
            KeyRing::from_special_id(KeyRingIdentifier::Session, false).map_err(decode_error)?;

        // Link the persistent keyring to the session
        let persistent = KeyRing::get_persistent(KeyRingIdentifier::Session).ok();

        // Construct the credential with a URI-style description
        let description = match target {
            Some("") => {
                return Err(ErrorCode::Invalid(
                    "target".to_string(),
                    "cannot be empty".to_string(),
                ));
            }
            Some(value) => value.to_string(),
            None => format!("keyring-rs:{user}@{service}"),
        };
        Ok(Self {
            session,
            persistent,
            description,
        })
    }
}

/// The builder for keyutils credentials
#[derive(Debug, Copy, Clone)]
struct KeyutilsCredentialBuilder {}

/// Return a keyutils credential builder.
///
/// If features are set to make keyutils the default store,
/// this will be automatically be called once before the
/// first credential is created.
pub fn default_credential_builder() -> Box<CredentialBuilder> {
    Box::new(KeyutilsCredentialBuilder {})
}

impl CredentialBuilderApi for KeyutilsCredentialBuilder {
    /// Build a keyutils credential with the given target, service, and user.
    ///
    /// Building a credential does not create a key in the store.
    /// It's setting a password that does that.
    fn build(&self, target: Option<&str>, service: &str, user: &str) -> Result<Box<Credential>> {
        Ok(Box::new(KeyutilsCredential::new_with_target(
            target, service, user,
        )?))
    }

    /// Return an [Any](std::any::Any) reference to the credential builder.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// Since this keystore keeps credentials in kernel memory,
    /// they vanish on reboot
    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::UntilReboot
    }
}

/// Map an underlying keyutils error to a platform-independent error with annotation.
pub fn decode_error(err: KeyError) -> ErrorCode {
    match err {
        // Experimentation has shown that the keyutils implementation can return a lot of
        // different errors that all mean "no such key", depending on where in the invalidation
        // processing the [get_password](KeyutilsCredential::get_password) call is made.
        KeyError::KeyDoesNotExist
        | KeyError::AccessDenied
        | KeyError::KeyRevoked
        | KeyError::KeyExpired => ErrorCode::NoEntry,
        KeyError::InvalidDescription => ErrorCode::Invalid(
            "description".to_string(),
            "rejected by platform".to_string(),
        ),
        KeyError::InvalidArguments => {
            ErrorCode::Invalid("password".to_string(), "rejected by platform".to_string())
        }
        other => ErrorCode::PlatformFailure(wrap(other)),
    }
}

fn wrap(err: KeyError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(err)
}

#[cfg(test)]
mod tests {
    use crate::credential::CredentialPersistence;
    use crate::{tests::generate_random_string, Entry, Error};

    use super::{default_credential_builder, KeyutilsCredential};

    #[test]
    fn test_persistence() {
        assert!(matches!(
            default_credential_builder().persistence(),
            CredentialPersistence::UntilReboot
        ))
    }

    fn entry_new(service: &str, user: &str) -> Entry {
        crate::tests::entry_from_constructor(KeyutilsCredential::new_with_target, service, user)
    }

    #[test]
    fn test_invalid_parameter() {
        let credential = KeyutilsCredential::new_with_target(Some(""), "service", "user");
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

    #[test]
    fn test_get_credential() {
        let name = generate_random_string();
        let entry = entry_new(&name, &name);
        let credential: &KeyutilsCredential = entry
            .get_credential()
            .downcast_ref()
            .expect("Not a Keyutils credential");
        assert!(
            credential.get_credential().is_err(),
            "Platform credential shouldn't exist yet!"
        );
        entry
            .set_password("test get_credential")
            .expect("Can't set password for get_credential");
        assert!(credential.get_credential().is_ok());
        entry
            .delete_credential()
            .expect("Couldn't delete after get_credential");
        assert!(matches!(entry.get_password(), Err(Error::NoEntry)));
    }
}
