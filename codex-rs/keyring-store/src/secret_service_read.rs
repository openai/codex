//! Headless reads must never ask the OS to display an unlock dialog.

use keyring::secret_service::SsCredential;
use keyring::secret_service::decode_error;
use secret_service::blocking;
use std::collections::HashMap;

pub(super) fn get_password(
    entry: &keyring::Entry,
    service: &str,
    account: &str,
) -> keyring::Result<String> {
    let has_display = ["DISPLAY", "WAYLAND_DISPLAY"]
        .iter()
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()));
    let Some(credential) = entry.get_credential().downcast_ref::<SsCredential>() else {
        return entry.get_password();
    };
    if has_display || credential.attributes.get("target").map(String::as_str) != Some("default") {
        return entry.get_password();
    }

    let store = blocking::SecretService::connect(secret_service::EncryptionType::Dh)
        .map_err(|error| keyring::Error::PlatformFailure(Box::new(error)))?;
    let search = store
        .search_items(HashMap::from([
            ("service", service),
            ("username", account),
            ("target", "default"),
        ]))
        .map_err(decode_error)?;
    let mut items = search.unlocked;
    items.extend(search.locked);
    let read = |item: &blocking::Item<'_>| {
        item.ensure_unlocked().map_err(decode_error)?;
        keyring::secret_service::get_item_password(item)
    };
    match items.as_slice() {
        // keyring's legacy lookup already reads without opening an unlock dialog.
        [] => credential
            .map_matching_legacy_items(&store, read, true)?
            .pop()
            .ok_or(keyring::Error::NoEntry),
        [item] => read(item),
        items => {
            let mut matches: Vec<Box<keyring::credential::Credential>> = Vec::new();
            for item in items {
                matches.push(Box::new(SsCredential::new_from_item(item)?));
            }
            Err(keyring::Error::Ambiguous(matches))
        }
    }
}
