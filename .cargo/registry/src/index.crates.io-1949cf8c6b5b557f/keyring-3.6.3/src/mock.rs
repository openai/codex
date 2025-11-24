/*!

# Mock credential store

To facilitate testing of clients, this crate provides a Mock credential store
that is platform-independent, provides no persistence, and allows the client
to specify the return values (including errors) for each call. The credentials
in this store have no attributes at all.

To use this credential store instead of the default, make this call during
application startup _before_ creating any entries:
```rust
# use keyring::{set_default_credential_builder, mock};
set_default_credential_builder(mock::default_credential_builder());
```

You can then create entries as you usually do, and call their usual methods
to set, get, and delete passwords.  There is no persistence other than
in the entry itself, so getting a password before setting it will always result
in a [NoEntry](Error::NoEntry) error.

If you want a method call on an entry to fail in a specific way, you can
downcast the entry to a [MockCredential] and then call [set_error](MockCredential::set_error)
with the appropriate error.  The next entry method called on the credential
will fail with the error you set.  The error will then be cleared, so the next
call on the mock will operate as usual.  Here's a complete example:
```rust
# use keyring::{Entry, Error, mock, mock::MockCredential};
# keyring::set_default_credential_builder(mock::default_credential_builder());
let entry = Entry::new("service", "user").unwrap();
let mock: &MockCredential = entry.get_credential().downcast_ref().unwrap();
mock.set_error(Error::Invalid("mock error".to_string(), "takes precedence".to_string()));
entry.set_password("test").expect_err("error will override");
entry.set_password("test").expect("error has been cleared");
```
 */
use std::cell::RefCell;
use std::sync::Mutex;

use super::credential::{
    Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi, CredentialPersistence,
};
use super::error::{decode_password, Error, Result};

/// The concrete mock credential
///
/// Mocks use an internal mutability pattern since entries are read-only.
/// The mutex is used to make sure these are Sync.
#[derive(Debug)]
pub struct MockCredential {
    pub inner: Mutex<RefCell<MockData>>,
}

impl Default for MockCredential {
    fn default() -> Self {
        Self {
            inner: Mutex::new(RefCell::new(Default::default())),
        }
    }
}

/// The (in-memory) persisted data for a mock credential.
///
/// We keep a password, but unlike most keystores
/// we also keep an intended error to return on the next call.
///
/// (Everything about this structure is public for transparency.
/// Most keystore implementation hide their internals.)
#[derive(Debug, Default)]
pub struct MockData {
    pub secret: Option<Vec<u8>>,
    pub error: Option<Error>,
}

impl CredentialApi for MockCredential {
    /// Set a password on a mock credential.
    ///
    /// If there is an error in the mock, it will be returned
    /// and the password will _not_ be set.  The error will
    /// be cleared, so calling again will set the password.
    fn set_password(&self, password: &str) -> Result<()> {
        let mut inner = self.inner.lock().expect("Can't access mock data for set");
        let data = inner.get_mut();
        let err = data.error.take();
        match err {
            None => {
                data.secret = Some(password.as_bytes().to_vec());
                Ok(())
            }
            Some(err) => Err(err),
        }
    }

    /// Set a password on a mock credential.
    ///
    /// If there is an error in the mock, it will be returned
    /// and the password will _not_ be set.  The error will
    /// be cleared, so calling again will set the password.
    fn set_secret(&self, secret: &[u8]) -> Result<()> {
        let mut inner = self.inner.lock().expect("Can't access mock data for set");
        let data = inner.get_mut();
        let err = data.error.take();
        match err {
            None => {
                data.secret = Some(secret.to_vec());
                Ok(())
            }
            Some(err) => Err(err),
        }
    }

    /// Get the password from a mock credential, if any.
    ///
    /// If there is an error set in the mock, it will
    /// be returned instead of a password.
    fn get_password(&self) -> Result<String> {
        let mut inner = self.inner.lock().expect("Can't access mock data for get");
        let data = inner.get_mut();
        let err = data.error.take();
        match err {
            None => match &data.secret {
                None => Err(Error::NoEntry),
                Some(val) => decode_password(val.clone()),
            },
            Some(err) => Err(err),
        }
    }

    /// Get the password from a mock credential, if any.
    ///
    /// If there is an error set in the mock, it will
    /// be returned instead of a password.
    fn get_secret(&self) -> Result<Vec<u8>> {
        let mut inner = self.inner.lock().expect("Can't access mock data for get");
        let data = inner.get_mut();
        let err = data.error.take();
        match err {
            None => match &data.secret {
                None => Err(Error::NoEntry),
                Some(val) => Ok(val.clone()),
            },
            Some(err) => Err(err),
        }
    }

    /// Delete the password in a mock credential
    ///
    /// If there is an error, it will be returned and
    /// the deletion will not happen.
    ///
    /// If there is no password, a [NoEntry](Error::NoEntry) error
    /// will be returned.
    fn delete_credential(&self) -> Result<()> {
        let mut inner = self
            .inner
            .lock()
            .expect("Can't access mock data for delete");
        let data = inner.get_mut();
        let err = data.error.take();
        match err {
            None => match data.secret {
                Some(_) => {
                    data.secret = None;
                    Ok(())
                }
                None => Err(Error::NoEntry),
            },
            Some(err) => Err(err),
        }
    }

    /// Return this mock credential concrete object
    /// wrapped in the [Any](std::any::Any) trait,
    /// so it can be downcast.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// Expose the concrete debug formatter for use via the [Credential] trait
    fn debug_fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl MockCredential {
    /// Make a new mock credential.
    ///
    /// Since mocks have no persistence between sessions,
    /// new mocks always have no password.
    fn new_with_target(_target: Option<&str>, _service: &str, _user: &str) -> Result<Self> {
        Ok(Default::default())
    }

    /// Set an error to be returned from this mock credential.
    ///
    /// Error returns always take precedence over the normal
    /// behavior of the mock.  But once an error has been
    /// returned it is removed, so the mock works thereafter.
    pub fn set_error(&self, err: Error) {
        let mut inner = self
            .inner
            .lock()
            .expect("Can't access mock data for set_error");
        let data = inner.get_mut();
        data.error = Some(err);
    }
}

/// The builder for mock credentials.
pub struct MockCredentialBuilder {}

impl CredentialBuilderApi for MockCredentialBuilder {
    /// Build a mock credential for the given target, service, and user.
    ///
    /// Since mocks don't persist between sessions,  all mocks
    /// start off without passwords.
    fn build(&self, target: Option<&str>, service: &str, user: &str) -> Result<Box<Credential>> {
        let credential = MockCredential::new_with_target(target, service, user)?;
        Ok(Box::new(credential))
    }

    /// Get an [Any][std::any::Any] reference to the mock credential builder.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    /// This keystore keeps the password in the entry!
    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::EntryOnly
    }
}

/// Return a mock credential builder for use by clients.
pub fn default_credential_builder() -> Box<CredentialBuilder> {
    Box::new(MockCredentialBuilder {})
}

#[cfg(test)]
mod tests {
    use super::{default_credential_builder, MockCredential};
    use crate::credential::CredentialPersistence;
    use crate::{tests::generate_random_string, Entry, Error};

    #[test]
    fn test_persistence() {
        assert!(matches!(
            default_credential_builder().persistence(),
            CredentialPersistence::EntryOnly
        ))
    }

    fn entry_new(service: &str, user: &str) -> Entry {
        let credential = MockCredential::new_with_target(None, service, user).unwrap();
        Entry::new_with_credential(Box::new(credential))
    }

    #[test]
    fn test_missing_entry() {
        crate::tests::test_missing_entry(entry_new);
    }

    #[test]
    fn test_empty_password() {
        crate::tests::test_empty_password(entry_new);
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
    fn test_get_update_attributes() {
        crate::tests::test_noop_get_update_attributes(entry_new);
    }

    #[test]
    fn test_set_error() {
        let name = generate_random_string();
        let entry = entry_new(&name, &name);
        let password = "test ascii password";
        let mock: &MockCredential = entry
            .inner
            .as_any()
            .downcast_ref()
            .expect("Downcast failed");
        mock.set_error(Error::Invalid(
            "mock error".to_string(),
            "is an error".to_string(),
        ));
        assert!(
            matches!(entry.set_password(password), Err(Error::Invalid(_, _))),
            "set: No error"
        );
        entry
            .set_password(password)
            .expect("set: Error not cleared");
        mock.set_error(Error::NoEntry);
        assert!(
            matches!(entry.get_password(), Err(Error::NoEntry)),
            "get: No error"
        );
        let stored_password = entry.get_password().expect("get: Error not cleared");
        assert_eq!(
            stored_password, password,
            "Retrieved and set ascii passwords don't match"
        );
        mock.set_error(Error::TooLong("mock".to_string(), 3));
        assert!(
            matches!(entry.delete_credential(), Err(Error::TooLong(_, 3))),
            "delete: No error"
        );
        entry
            .delete_credential()
            .expect("delete: Error not cleared");
        assert!(
            matches!(entry.get_password(), Err(Error::NoEntry)),
            "Able to read a deleted ascii password"
        )
    }
}
