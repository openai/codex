use cookie_store::CookieStore;
use std::ops::Deref;
use std::sync::{RwLock, RwLockReadGuard};
use url::Url;

#[derive(Debug)]
pub(crate) struct CookieTin {
    inner: RwLock<CookieStore>,
}

/// RAII guard for read access to the CookieStore.
pub struct CookieStoreGuard<'a>(RwLockReadGuard<'a, CookieStore>);

impl<'a> Deref for CookieStoreGuard<'a> {
    type Target = CookieStore;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CookieTin {
    pub(crate) fn new(store: CookieStore) -> Self {
        CookieTin {
            inner: RwLock::new(store),
        }
    }

    pub(crate) fn read_lock(&self) -> CookieStoreGuard<'_> {
        let lock = self.inner.read().unwrap();
        CookieStoreGuard(lock)
    }

    pub(crate) fn get_request_cookies(&self, url: &Url) -> Vec<cookie::Cookie> {
        let store = self.inner.read().unwrap();
        store
            .get_request_values(url)
            .map(|(name, value)| cookie::Cookie::new(name.to_owned(), value.to_owned()))
            .collect()
    }

    pub(crate) fn store_response_cookies<I>(&self, cookies: I, url: &Url)
    where
        I: Iterator<Item = cookie::Cookie<'static>>,
    {
        let mut store = self.inner.write().unwrap();
        store.store_response_cookies(cookies, url);
    }
}
