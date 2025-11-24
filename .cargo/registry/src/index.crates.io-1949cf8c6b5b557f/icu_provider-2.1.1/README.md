# icu_provider [![crates.io](https://img.shields.io/crates/v/icu_provider)](https://crates.io/crates/icu_provider)

<!-- cargo-rdme start -->

`icu_provider` is one of the `ICU4X` components.

Unicode's experience with ICU4X's parent projects, ICU4C and ICU4J, led the team to realize
that data management is the most critical aspect of deploying internationalization, and that it requires
a high level of customization for the needs of the platform it is embedded in. As a result
ICU4X comes with a selection of providers that should allow for ICU4X to naturally fit into
different business and technological needs of customers.

`icu_provider` defines traits and structs for transmitting data through the ICU4X locale
data pipeline. The primary trait is [`DataProvider`]. It is parameterized by a
[`DataMarker`], which is the type-system-level data identifier. [`DataProvider`] has a single method,
[`DataProvider::load`], which transforms a [`DataRequest`] into a [`DataResponse`].

- [`DataRequest`] contains selectors to choose a specific variant of the marker, such as a locale.
- [`DataResponse`] contains the data if the request was successful.

The most common types required for this crate are included via the prelude:

```rust
use icu_provider::prelude::*;
```

### Dynamic Data Providers

If the type system cannot be leveraged to load data (such as when dynamically loading from I/O),
there's another form of the [`DataProvider`]: [`DynamicDataProvider`]. While [`DataProvider`] is parametrized
on the type-system level by a [`DataMarker`] (which are distinct types implementing this trait),
[`DynamicDataProvider`]s are parametrized at runtime by a [`DataMarkerInfo`] struct, which essentially is the runtime
representation of the [`DataMarker`] type.

The [`DynamicDataProvider`] is still type-level parametrized by the type that it loads, and there are two
implementations that should be called out

- [`DynamicDataProvider<BufferMarker>`], a.k.a. [`BufferProvider`](buf::BufferProvider) returns data as `[u8]` buffers.

#### BufferProvider

These providers are able to return unstructured data typically represented as
[`serde`]-serialized buffers. Users can call [`as_deserializing()`] to get an object
implementing [`DataProvider`] by invoking Serde Deserialize.

Examples of BufferProviders:

- [`FsDataProvider`] reads individual buffers from the filesystem.
- [`BlobDataProvider`] reads buffers from a large in-memory blob.

### Provider Adapters

ICU4X offers several built-in modules to combine providers in interesting ways.
These can be found in the [`icu_provider_adapters`] crate.

### Testing Provider

This crate also contains a concrete provider for demonstration purposes:

- [`HelloWorldProvider`] returns "hello world" strings in several languages.

### Types and Lifetimes

Types compatible with [`Yokeable`] can be passed through the data provider, so long as they are
associated with a marker type implementing [`DynamicDataMarker`].

Data structs should generally have one lifetime argument: `'data`. This lifetime allows data
structs to borrow zero-copy data.

[`FixedProvider`]: https://docs.rs/icu_provider_adapters/latest/fixed/any_payload/struct.FixedProvider.html
[`HelloWorldProvider`]: hello_world::HelloWorldProvider
[`Yokeable`]: yoke::Yokeable
[`impl_dynamic_data_provider!`]: dynutil::impl_dynamic_data_provider
[`icu_provider_adapters`]: https://docs.rs/icu_provider_adapters/latest/icu_provider_adapters/index.html
[`SourceDataProvider`]: https://docs.rs/icu_provider_source/latest/icu_provider_source/struct.SourceDataProvider.html
[`as_deserializing()`]: buf::AsDeserializingBufferProvider::as_deserializing
[`FsDataProvider`]: https://docs.rs/icu_provider_fs/latest/icu_provider_fs/struct.FsDataProvider.html
[`BlobDataProvider`]: https://docs.rs/icu_provider_blob/latest/icu_provider_blob/struct.BlobDataProvider.html

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
