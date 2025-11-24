use std::{collections::HashSet, marker::PhantomData, sync::Arc};

use static_assertions::assert_impl_all;
use zbus_names::{BusName, InterfaceName};
use zvariant::{ObjectPath, Str};

use crate::{proxy::ProxyInner, Connection, Error, Proxy, Result};

/// The properties caching mode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheProperties {
    /// Cache properties. The properties will be cached upfront as part of the proxy
    /// creation.
    Yes,
    /// Don't cache properties.
    No,
    /// Cache properties but only populate the cache on the first read of a property (default).
    #[default]
    Lazily,
}

/// Builder for proxies.
#[derive(Debug)]
pub struct Builder<'a, T = ()> {
    conn: Connection,
    destination: Option<BusName<'a>>,
    path: Option<ObjectPath<'a>>,
    interface: Option<InterfaceName<'a>>,
    proxy_type: PhantomData<T>,
    cache: CacheProperties,
    uncached_properties: Option<HashSet<Str<'a>>>,
}

impl<'a, T> Clone for Builder<'a, T> {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
            destination: self.destination.clone(),
            path: self.path.clone(),
            interface: self.interface.clone(),
            cache: self.cache,
            uncached_properties: self.uncached_properties.clone(),
            proxy_type: PhantomData,
        }
    }
}

assert_impl_all!(Builder<'_>: Send, Sync, Unpin);

impl<'a, T> Builder<'a, T> {
    /// Set the proxy destination address.
    pub fn destination<D>(mut self, destination: D) -> Result<Self>
    where
        D: TryInto<BusName<'a>>,
        D::Error: Into<Error>,
    {
        self.destination = Some(destination.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Set the proxy path.
    pub fn path<P>(mut self, path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'a>>,
        P::Error: Into<Error>,
    {
        self.path = Some(path.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Set the proxy interface.
    pub fn interface<I>(mut self, interface: I) -> Result<Self>
    where
        I: TryInto<InterfaceName<'a>>,
        I::Error: Into<Error>,
    {
        self.interface = Some(interface.try_into().map_err(Into::into)?);
        Ok(self)
    }

    /// Set the properties caching mode.
    #[must_use]
    pub fn cache_properties(mut self, cache: CacheProperties) -> Self {
        self.cache = cache;
        self
    }

    /// Specify a set of properties (by name) which should be excluded from caching.
    #[must_use]
    pub fn uncached_properties(mut self, properties: &[&'a str]) -> Self {
        self.uncached_properties
            .replace(properties.iter().map(|p| Str::from(*p)).collect());

        self
    }

    pub(crate) fn build_internal(self) -> Result<Proxy<'a>> {
        let conn = self.conn;
        let destination = self
            .destination
            .ok_or(Error::MissingParameter("destination"))?;
        let path = self.path.ok_or(Error::MissingParameter("path"))?;
        let interface = self.interface.ok_or(Error::MissingParameter("interface"))?;
        let cache = self.cache;
        let uncached_properties = self.uncached_properties.unwrap_or_default();

        Ok(Proxy {
            inner: Arc::new(ProxyInner::new(
                conn,
                destination,
                path,
                interface,
                cache,
                uncached_properties,
            )),
        })
    }

    /// Build a proxy from the builder.
    ///
    /// # Errors
    ///
    /// If the builder is lacking the necessary parameters to build a proxy,
    /// [`Error::MissingParameter`] is returned.
    pub async fn build(self) -> Result<T>
    where
        T: From<Proxy<'a>>,
    {
        let cache_upfront = self.cache == CacheProperties::Yes;
        let proxy = self.build_internal()?;

        if cache_upfront {
            proxy
                .get_property_cache()
                .expect("properties cache not initialized")
                .ready()
                .await?;
        }

        Ok(proxy.into())
    }
}

impl<'a, T> Builder<'a, T>
where
    T: ProxyDefault,
{
    /// Create a new [`Builder`] for the given connection.
    #[must_use]
    pub fn new(conn: &Connection) -> Self {
        Self {
            conn: conn.clone(),
            destination: T::DESTINATION
                .map(|d| BusName::from_static_str(d).expect("invalid bus name")),
            path: T::PATH.map(|p| ObjectPath::from_static_str(p).expect("invalid default path")),
            interface: T::INTERFACE
                .map(|i| InterfaceName::from_static_str(i).expect("invalid interface name")),
            cache: CacheProperties::default(),
            uncached_properties: None,
            proxy_type: PhantomData,
        }
    }

    /// Create a new [`Builder`] for the given connection.
    #[must_use]
    #[deprecated(
        since = "4.0.0",
        note = "use `Builder::new` instead, which is now generic over the proxy type"
    )]
    pub fn new_bare(conn: &Connection) -> Self {
        Self::new(conn)
    }
}

/// Trait for the default associated values of a proxy.
///
/// The trait is automatically implemented by the [`dbus_proxy`] macro on your behalf, and may be
/// later used to retrieve the associated constants.
///
/// [`dbus_proxy`]: attr.dbus_proxy.html
pub trait ProxyDefault {
    const INTERFACE: Option<&'static str>;
    const DESTINATION: Option<&'static str>;
    const PATH: Option<&'static str>;
}

impl ProxyDefault for Proxy<'_> {
    const INTERFACE: Option<&'static str> = None;
    const DESTINATION: Option<&'static str> = None;
    const PATH: Option<&'static str> = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    #[ntest::timeout(15000)]
    fn builder() {
        crate::utils::block_on(builder_async());
    }

    async fn builder_async() {
        let conn = Connection::session().await.unwrap();

        let builder = Builder::<Proxy<'_>>::new(&conn)
            .destination("org.freedesktop.DBus")
            .unwrap()
            .path("/some/path")
            .unwrap()
            .interface("org.freedesktop.Interface")
            .unwrap()
            .cache_properties(CacheProperties::No);
        assert!(matches!(
            builder.clone().destination.unwrap(),
            BusName::Unique(_),
        ));
        let proxy = builder.build().await.unwrap();
        assert!(matches!(proxy.inner.destination, BusName::Unique(_)));
    }
}
