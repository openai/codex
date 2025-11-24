// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Data provider returning multilingual "Hello World" strings for testing.

#![allow(clippy::exhaustive_structs)] // data struct module

use crate as icu_provider;

use crate::prelude::*;
use alloc::borrow::Cow;
use alloc::collections::BTreeSet;
use alloc::string::String;
use core::fmt::Debug;
use icu_locale_core::preferences::define_preferences;
use writeable::Writeable;
use yoke::*;
use zerofrom::*;

/// A struct containing "Hello World" in the requested language.
#[derive(Debug, PartialEq, Clone, Yokeable, ZeroFrom)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(
    any(feature = "deserialize_json", feature = "export"),
    derive(serde::Serialize)
)]
#[cfg_attr(feature = "export", derive(databake::Bake))]
#[cfg_attr(feature = "export", databake(path = icu_provider::hello_world))]
pub struct HelloWorld<'data> {
    /// The translation of "Hello World".
    #[cfg_attr(feature = "serde", serde(borrow))]
    pub message: Cow<'data, str>,
}

impl Default for HelloWorld<'_> {
    fn default() -> Self {
        HelloWorld {
            message: Cow::Borrowed("(und) Hello World"),
        }
    }
}

impl<'a> ZeroFrom<'a, str> for HelloWorld<'a> {
    fn zero_from(message: &'a str) -> Self {
        HelloWorld {
            message: Cow::Borrowed(message),
        }
    }
}

crate::data_struct!(
    HelloWorld<'data>,
    varule: str,
    #[cfg(feature = "export")]
    encode_as_varule: |v: &HelloWorld<'_>| &*v.message
);

data_marker!(
    /// Marker type for [`HelloWorld`].
    #[derive(Debug)]
    HelloWorldV1,
    HelloWorld<'static>,
    has_checksum = true,
);

/// A data provider returning Hello World strings in different languages.
///
/// Mostly useful for testing.
///
/// # Examples
///
/// ```
/// use icu_locale_core::langid;
/// use icu_provider::hello_world::*;
/// use icu_provider::prelude::*;
///
/// let german_hello_world: DataResponse<HelloWorldV1> = HelloWorldProvider
///     .load(DataRequest {
///         id: DataIdentifierBorrowed::for_locale(&langid!("de").into()),
///         ..Default::default()
///     })
///     .expect("Loading should succeed");
///
/// assert_eq!("Hallo Welt", german_hello_world.payload.get().message);
/// ```
///
/// Load the reverse string using an auxiliary key:
///
/// ```
/// use icu_locale_core::langid;
/// use icu_provider::hello_world::*;
/// use icu_provider::prelude::*;
///
/// let reverse_hello_world: DataResponse<HelloWorldV1> = HelloWorldProvider
///     .load(DataRequest {
///         id: DataIdentifierBorrowed::for_marker_attributes_and_locale(
///             DataMarkerAttributes::from_str_or_panic("reverse"),
///             &langid!("en").into(),
///         ),
///         ..Default::default()
///     })
///     .expect("Loading should succeed");
///
/// assert_eq!("Olleh Dlrow", reverse_hello_world.payload.get().message);
/// ```
#[derive(Debug, PartialEq, Default)]
pub struct HelloWorldProvider;

impl HelloWorldProvider {
    // Data from https://en.wiktionary.org/wiki/Hello_World#Translations
    // Keep this sorted!
    const DATA: &'static [(&'static str, &'static str, &'static str)] = &[
        ("bn", "", "ওহে বিশ্ব"),
        ("cs", "", "Ahoj světe"),
        ("de", "", "Hallo Welt"),
        ("de-AT", "", "Servus Welt"),
        ("el", "", "Καλημέρα κόσμε"),
        ("en", "", "Hello World"),
        // WORLD
        ("en-001", "", "Hello from 🗺️"),
        // AFRICA
        ("en-002", "", "Hello from 🌍"),
        // AMERICAS
        ("en-019", "", "Hello from 🌎"),
        // ASIA
        ("en-142", "", "Hello from 🌏"),
        // GREAT BRITAIN
        ("en-GB", "", "Hello from 🇬🇧"),
        // ENGLAND
        ("en-GB-u-sd-gbeng", "", "Hello from 🏴󠁧󠁢󠁥󠁮󠁧󠁿"),
        ("en", "reverse", "Olleh Dlrow"),
        ("eo", "", "Saluton, Mondo"),
        ("fa", "", "سلام دنیا‎"),
        ("fi", "", "hei maailma"),
        ("is", "", "Halló, heimur"),
        ("ja", "", "こんにちは世界"),
        ("ja", "reverse", "界世はちにんこ"),
        ("la", "", "Ave, munde"),
        ("pt", "", "Olá, mundo"),
        ("ro", "", "Salut, lume"),
        ("ru", "", "Привет, мир"),
        ("sr", "", "Поздрав свете"),
        ("sr-Latn", "", "Pozdrav svete"),
        ("vi", "", "Xin chào thế giới"),
        ("zh", "", "你好世界"),
    ];

    /// Converts this provider into a [`BufferProvider`] that uses JSON serialization.
    #[cfg(feature = "deserialize_json")]
    pub fn into_json_provider(self) -> HelloWorldJsonProvider {
        HelloWorldJsonProvider
    }
}

impl DataProvider<HelloWorldV1> for HelloWorldProvider {
    fn load(&self, req: DataRequest) -> Result<DataResponse<HelloWorldV1>, DataError> {
        let data = Self::DATA
            .iter()
            .find(|(l, a, _)| {
                req.id.locale.strict_cmp(l.as_bytes()).is_eq()
                    && *a == req.id.marker_attributes.as_str()
            })
            .map(|(_, _, v)| v)
            .ok_or_else(|| DataErrorKind::IdentifierNotFound.with_req(HelloWorldV1::INFO, req))?;
        Ok(DataResponse {
            metadata: DataResponseMetadata::default().with_checksum(1234),
            payload: DataPayload::from_static_str(data),
        })
    }
}

impl DryDataProvider<HelloWorldV1> for HelloWorldProvider {
    fn dry_load(&self, req: DataRequest) -> Result<DataResponseMetadata, DataError> {
        self.load(req).map(|r| r.metadata)
    }
}

impl DataPayload<HelloWorldV1> {
    /// Make a [`DataPayload`]`<`[`HelloWorldV1`]`>` from a static string slice.
    pub fn from_static_str(s: &'static str) -> DataPayload<HelloWorldV1> {
        DataPayload::from_owned(HelloWorld {
            message: Cow::Borrowed(s),
        })
    }
}

#[cfg(feature = "deserialize_json")]
/// A data provider returning Hello World strings in different languages as JSON blobs.
///
/// Mostly useful for testing.
///
/// # Examples
///
/// ```
/// use icu_locale_core::langid;
/// use icu_provider::hello_world::*;
/// use icu_provider::prelude::*;
///
/// let german_hello_world = HelloWorldProvider
///     .into_json_provider()
///     .load_data(HelloWorldV1::INFO, DataRequest {
///         id: DataIdentifierBorrowed::for_locale(&langid!("de").into()),
///         ..Default::default()
///     })
///     .expect("Loading should succeed");
///
/// assert_eq!(german_hello_world.payload.get(), br#"{"message":"Hallo Welt"}"#);
#[derive(Debug)]
pub struct HelloWorldJsonProvider;

#[cfg(feature = "deserialize_json")]
impl DynamicDataProvider<BufferMarker> for HelloWorldJsonProvider {
    fn load_data(
        &self,
        marker: DataMarkerInfo,
        req: DataRequest,
    ) -> Result<DataResponse<BufferMarker>, DataError> {
        marker.match_marker(HelloWorldV1::INFO)?;
        let result = HelloWorldProvider.load(req)?;
        Ok(DataResponse {
            metadata: DataResponseMetadata {
                buffer_format: Some(icu_provider::buf::BufferFormat::Json),
                ..result.metadata
            },
            #[expect(clippy::unwrap_used)] // HelloWorld::serialize is infallible
            payload: DataPayload::from_owned_buffer(
                serde_json::to_string(result.payload.get())
                    .unwrap()
                    .into_bytes()
                    .into_boxed_slice(),
            ),
        })
    }
}

impl IterableDataProvider<HelloWorldV1> for HelloWorldProvider {
    fn iter_ids(&self) -> Result<BTreeSet<DataIdentifierCow<'_>>, DataError> {
        #[expect(clippy::unwrap_used)] // hello-world
        Ok(Self::DATA
            .iter()
            .map(|(l, a, _)| {
                DataIdentifierCow::from_borrowed_and_owned(
                    DataMarkerAttributes::from_str_or_panic(a),
                    l.parse().unwrap(),
                )
            })
            .collect())
    }
}

#[cfg(feature = "export")]
icu_provider::export::make_exportable_provider!(HelloWorldProvider, [HelloWorldV1,]);

define_preferences!(
    /// Hello World Preferences.
    [Copy]
    HelloWorldFormatterPreferences, {}
);

/// A type that formats localized "hello world" strings.
///
/// This type is intended to take the shape of a typical ICU4X formatter API.
///
/// # Examples
///
/// ```
/// use icu_locale_core::locale;
/// use icu_provider::hello_world::{HelloWorldFormatter, HelloWorldProvider};
/// use writeable::assert_writeable_eq;
///
/// let fmt = HelloWorldFormatter::try_new_unstable(
///     &HelloWorldProvider,
///     locale!("eo").into(),
/// )
/// .expect("locale exists");
///
/// assert_writeable_eq!(fmt.format(), "Saluton, Mondo");
/// ```
#[derive(Debug)]
pub struct HelloWorldFormatter {
    data: DataPayload<HelloWorldV1>,
}

/// A formatted hello world message. Implements [`Writeable`].
///
/// For an example, see [`HelloWorldFormatter`].
#[derive(Debug)]
pub struct FormattedHelloWorld<'l> {
    data: &'l HelloWorld<'l>,
}

impl HelloWorldFormatter {
    /// Creates a new [`HelloWorldFormatter`] for the specified locale.
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    pub fn try_new(prefs: HelloWorldFormatterPreferences) -> Result<Self, DataError> {
        Self::try_new_unstable(&HelloWorldProvider, prefs)
    }

    icu_provider::gen_buffer_data_constructors!((prefs: HelloWorldFormatterPreferences) -> error: DataError,
        functions: [
            try_new: skip,
            try_new_with_buffer_provider,
            try_new_unstable,
            Self,
    ]);

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::try_new)]
    pub fn try_new_unstable<P>(
        provider: &P,
        prefs: HelloWorldFormatterPreferences,
    ) -> Result<Self, DataError>
    where
        P: DataProvider<HelloWorldV1>,
    {
        let locale = HelloWorldV1::make_locale(prefs.locale_preferences);
        let data = provider
            .load(DataRequest {
                id: crate::request::DataIdentifierBorrowed::for_locale(&locale),
                ..Default::default()
            })?
            .payload;
        Ok(Self { data })
    }

    /// Formats a hello world message, returning a [`FormattedHelloWorld`].
    pub fn format<'l>(&'l self) -> FormattedHelloWorld<'l> {
        FormattedHelloWorld {
            data: self.data.get(),
        }
    }

    /// Formats a hello world message, returning a [`String`].
    pub fn format_to_string(&self) -> String {
        self.format().write_to_string().into_owned()
    }
}

impl Writeable for FormattedHelloWorld<'_> {
    fn write_to<W: core::fmt::Write + ?Sized>(&self, sink: &mut W) -> core::fmt::Result {
        self.data.message.write_to(sink)
    }

    fn writeable_borrow(&self) -> Option<&str> {
        self.data.message.writeable_borrow()
    }

    fn writeable_length_hint(&self) -> writeable::LengthHint {
        self.data.message.writeable_length_hint()
    }
}

writeable::impl_display_with_writeable!(FormattedHelloWorld<'_>);

#[cfg(feature = "export")]
#[test]
fn test_iter() {
    use crate::IterableDataProvider;
    use icu_locale_core::locale;

    assert_eq!(
        HelloWorldProvider.iter_ids().unwrap(),
        BTreeSet::from_iter([
            DataIdentifierCow::from_locale(locale!("bn").into()),
            DataIdentifierCow::from_locale(locale!("cs").into()),
            DataIdentifierCow::from_locale(locale!("de").into()),
            DataIdentifierCow::from_locale(locale!("de-AT").into()),
            DataIdentifierCow::from_locale(locale!("el").into()),
            DataIdentifierCow::from_locale(locale!("en").into()),
            DataIdentifierCow::from_locale(locale!("en-001").into()),
            DataIdentifierCow::from_locale(locale!("en-002").into()),
            DataIdentifierCow::from_locale(locale!("en-019").into()),
            DataIdentifierCow::from_locale(locale!("en-142").into()),
            DataIdentifierCow::from_locale(locale!("en-GB").into()),
            DataIdentifierCow::from_locale(locale!("en-GB-u-sd-gbeng").into()),
            DataIdentifierCow::from_borrowed_and_owned(
                DataMarkerAttributes::from_str_or_panic("reverse"),
                locale!("en").into()
            ),
            DataIdentifierCow::from_locale(locale!("eo").into()),
            DataIdentifierCow::from_locale(locale!("fa").into()),
            DataIdentifierCow::from_locale(locale!("fi").into()),
            DataIdentifierCow::from_locale(locale!("is").into()),
            DataIdentifierCow::from_locale(locale!("ja").into()),
            DataIdentifierCow::from_borrowed_and_owned(
                DataMarkerAttributes::from_str_or_panic("reverse"),
                locale!("ja").into()
            ),
            DataIdentifierCow::from_locale(locale!("la").into()),
            DataIdentifierCow::from_locale(locale!("pt").into()),
            DataIdentifierCow::from_locale(locale!("ro").into()),
            DataIdentifierCow::from_locale(locale!("ru").into()),
            DataIdentifierCow::from_locale(locale!("sr").into()),
            DataIdentifierCow::from_locale(locale!("sr-Latn").into()),
            DataIdentifierCow::from_locale(locale!("vi").into()),
            DataIdentifierCow::from_locale(locale!("zh").into()),
        ])
    );
}
