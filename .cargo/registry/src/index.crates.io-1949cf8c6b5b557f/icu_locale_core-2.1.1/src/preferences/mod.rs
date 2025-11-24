// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This API provides necessary functionality for building user preferences structs.
//!
//! It includes the ability to merge information between the struct and a [`Locale`],
//! facilitating the resolution of attributes against default values.
//!
//! Preferences struct serve as a composable argument to `ICU4X` constructors, allowing
//! for ergonomic merging between information encoded in multiple sets of user inputs:
//! Locale, application preferences and operating system preferences.
//!
//! The crate is intended primarily to be used by components constructors to normalize the format
//! of ingesting preferences across all of `ICU4X`.
//!
//! # Preferences vs Options
//!
//! ICU4X introduces a separation between two classes of parameters that are used
//! to adjust the behavior of a component.
//!
//! `Preferences` represent the user-driven preferences on how the given user wants the internationalization
//! to behave. Those are items like language, script, calendar and numbering systems etc.
//!
//! `Options` represent the developer-driven adjustments that affect how given information is presented
//! based on the requirements of the application like available space or intended tone.
//!
//! # Options Division
//!
//! The `Options` themselves are also divided into options that are affecting data slicing, and ones that don't.
//! This is necessary to allow for DCE and FFI to produce minimal outputs avoiding loading unnecessary data that
//! is never to be used by a given component.
//! The result is that some option keys affect specialized constructors such as `try_new_short`, `try_new_long`, which
//! result in data provider loading only data necessary to format short or long values respectively.
//! For options that are not affecting data slicing, an `Options` struct is provided that the developer
//! can fill with selected key values, or use the defaults.
//!
//! # Preferences Merging
//!
//! In traditional internationalization APIs, the argument passed to constructors is a locale.
//! ICU4X changes this paradigm by accepting a `Preferences`, which can be extracted from a [`Locale`] and combined with
//! other `Preferences`s provided by the environment.
//!
//! This approach makes it easy for developers to write code that takes just a locale, as in other systems,
//! as well as handle more sophisticated cases where the application may receive, for example, a locale,
//! a set of internationalization preferences specified within the application,
//! and a third set extracted from the operating system's preferences.
//!
//! # ECMA-402 vs ICU4X
//!
//! The result of the two paradigm shifts presented above is that the way constructors work is different.
//!
//! ## ECMA-402
//! ```ignore
//! let locale = new Locale("en-US-u-hc-h12");
//! let options = {
//!   hourCycle: "h24", // user preference
//!   timeStyle: "long", // developer option
//! };
//!
//! let dtf = new DateTimeFormat(locale, options);
//! ```
//!
//! ## ICU4X
//! ```ignore
//! let loc = locale!("en-US-u-hc-h12");
//! let prefs = DateTimeFormatterPreferences {
//!     hour_cycle: HourCycle::H23,
//! };
//! let options = DateTimeFormatterOptions {
//!     time_style: TimeStyle::Long,
//! };
//!
//! let mut combined_prefs = DateTimeFormatterPreferences::from(loc);
//! combined_prefs.extend(prefs);
//!
//! let dtf = DateTimeFormatter::try_new(combined_prefs, options);
//! ```
//!
//! This architecture allows for flexible composition of user and developer settings
//! sourced from different locations in custom ways based on the needs of each deployment.
//!
//! Below are some examples of how the `Preferences` model can be used in different setups.
//!
//! # Examples
//!
//! ```
//! use icu::locale::preferences::{
//!   define_preferences,
//!   extensions::unicode::keywords::HourCycle,
//! };
//! use icu::locale::locale;
//!
//! # fn get_data_locale_from_prefs(input: ExampleComponentPreferences) -> () { () }
//! # fn load_data(locale: ()) -> MyData { MyData {} }
//! # struct MyData {}
//! define_preferences!(
//!     /// Name of the preferences struct
//!     [Copy]
//!     ExampleComponentPreferences,
//!     {
//!         /// A preference relevant to the component
//!         hour_cycle: HourCycle
//!     }
//! );
//!
//! pub struct ExampleComponent {
//!     data: MyData,
//! }
//!
//! impl ExampleComponent {
//!     pub fn new(prefs: ExampleComponentPreferences) -> Self {
//!         let locale = get_data_locale_from_prefs(prefs);
//!         let data = load_data(locale);
//!
//!         Self { data }
//!     }
//! }
//! ```
//!
//! Now we can use that component in multiple different ways,
//!
//! ## Scenario 1: Use Locale as the only input
//! ```
//! # use icu::locale::preferences::{
//! #   define_preferences,
//! #   extensions::unicode::keywords::HourCycle,
//! # };
//! # use icu::locale::locale;
//! # fn get_data_locale_from_prefs(input: ExampleComponentPreferences) -> () { () }
//! # fn load_data(locale: ()) -> MyData { MyData {} }
//! # struct MyData {}
//! # define_preferences!(
//! #     /// Name of the preferences struct
//! #     [Copy]
//! #     ExampleComponentPreferences,
//! #     {
//! #         /// A preference relevant to the component
//! #         hour_cycle: HourCycle
//! #     }
//! # );
//! #
//! # pub struct ExampleComponent {
//! #     data: MyData,
//! # }
//! # impl ExampleComponent {
//! #     pub fn new(prefs: ExampleComponentPreferences) -> Self {
//! #         let locale = get_data_locale_from_prefs(prefs);
//! #         let data = load_data(locale);
//! #
//! #         Self { data }
//! #     }
//! # }
//! let loc = locale!("en-US-u-hc-h23");
//! let tf = ExampleComponent::new(loc.into());
//! ```
//!
//! ## Scenario 2: Compose Preferences and Locale
//! ```
//! # use icu::locale::preferences::{
//! #   define_preferences,
//! #   extensions::unicode::keywords::HourCycle,
//! # };
//! # use icu::locale::locale;
//! # fn get_data_locale_from_prefs(input: ExampleComponentPreferences) -> () { () }
//! # fn load_data(locale: ()) -> MyData { MyData {} }
//! # struct MyData {}
//! # define_preferences!(
//! #     /// Name of the preferences struct
//! #     [Copy]
//! #     ExampleComponentPreferences,
//! #     {
//! #         /// A preference relevant to the component
//! #         hour_cycle: HourCycle
//! #     }
//! # );
//! #
//! # pub struct ExampleComponent {
//! #     data: MyData,
//! # }
//! # impl ExampleComponent {
//! #     pub fn new(prefs: ExampleComponentPreferences) -> Self {
//! #         let locale = get_data_locale_from_prefs(prefs);
//! #         let data = load_data(locale);
//! #
//! #         Self { data }
//! #     }
//! # }
//! let loc = locale!("en-US-u-hc-h23");
//! let app_prefs = ExampleComponentPreferences {
//!     hour_cycle: Some(HourCycle::H12),
//!     ..Default::default()
//! };
//!
//! let mut combined_prefs = ExampleComponentPreferences::from(loc);
//! combined_prefs.extend(app_prefs);
//!
//! // HourCycle is set from the prefs bag and override the value from the locale
//! assert_eq!(combined_prefs.hour_cycle, Some(HourCycle::H12));
//!
//! let tf = ExampleComponent::new(combined_prefs);
//! ```
//!
//! ## Scenario 3: Merge Preferences from Locale, OS, and Application
//! ```
//! # use icu::locale::preferences::{
//! #   define_preferences,
//! #   extensions::unicode::keywords::HourCycle,
//! # };
//! # use icu::locale::locale;
//! # fn get_data_locale_from_prefs(input: ExampleComponentPreferences) -> () { () }
//! # fn load_data(locale: ()) -> MyData { MyData {} }
//! # struct MyData {}
//! # define_preferences!(
//! #     /// Name of the preferences struct
//! #     [Copy]
//! #     ExampleComponentPreferences,
//! #     {
//! #         /// A preference relevant to the component
//! #         hour_cycle: HourCycle
//! #     }
//! # );
//! #
//! # pub struct ExampleComponent {
//! #     data: MyData,
//! # }
//! # impl ExampleComponent {
//! #     pub fn new(prefs: ExampleComponentPreferences) -> Self {
//! #         let locale = get_data_locale_from_prefs(prefs);
//! #         let data = load_data(locale);
//! #
//! #         Self { data }
//! #     }
//! # }
//! let loc = locale!("en-US");
//!
//! // Simulate OS preferences
//! let os_prefs = ExampleComponentPreferences {
//!     hour_cycle: Some(HourCycle::H23),
//!     ..Default::default()
//! };
//!
//! // Application does not specify hour_cycle
//! let app_prefs = ExampleComponentPreferences {
//!     hour_cycle: None,
//!     ..Default::default()
//! };
//!
//! let mut combined_prefs = ExampleComponentPreferences::from(loc);
//! combined_prefs.extend(os_prefs);
//! combined_prefs.extend(app_prefs);
//!
//! // HourCycle is set from the OS preferences since the application didn't specify it
//! assert_eq!(combined_prefs.hour_cycle, Some(HourCycle::H23));
//!
//! let tf = ExampleComponent::new(combined_prefs);
//! ```
//!
//! ## Scenario 4: Neither Application nor OS specify the preference
//! ```
//! # use icu::locale::preferences::{
//! #   define_preferences,
//! #   extensions::unicode::keywords::HourCycle,
//! # };
//! # use icu::locale::locale;
//! # fn get_data_locale_from_prefs(input: ExampleComponentPreferences) -> () { () }
//! # fn load_data(locale: ()) -> MyData { MyData {} }
//! # struct MyData {}
//! # define_preferences!(
//! #     /// Name of the preferences struct
//! #     [Copy]
//! #     ExampleComponentPreferences,
//! #     {
//! #         /// A preference relevant to the component
//! #         hour_cycle: HourCycle
//! #     }
//! # );
//! #
//! # pub struct ExampleComponent {
//! #     data: MyData,
//! # }
//! # impl ExampleComponent {
//! #     pub fn new(prefs: ExampleComponentPreferences) -> Self {
//! #         let locale = get_data_locale_from_prefs(prefs);
//! #         let data = load_data(locale);
//! #
//! #         Self { data }
//! #     }
//! # }
//! let loc = locale!("en-US-u-hc-h23");
//!
//! // Simulate OS preferences
//! let os_prefs = ExampleComponentPreferences::default(); // OS does not specify hour_cycle
//! let app_prefs = ExampleComponentPreferences::default(); // Application does not specify hour_cycle
//!
//! let mut combined_prefs = ExampleComponentPreferences::from(loc);
//! combined_prefs.extend(os_prefs);
//! combined_prefs.extend(app_prefs);
//!
//! // HourCycle is taken from the locale
//! assert_eq!(combined_prefs.hour_cycle, Some(HourCycle::H23));
//!
//! let tf = ExampleComponent::new(combined_prefs);
//! ```
//!
//! [`ICU4X`]: ../icu/index.html
//! [`Locale`]: crate::Locale

pub mod extensions;
mod locale;
pub use locale::*;

/// A low-level trait implemented on each preference exposed in component preferences.
///
/// [`PreferenceKey`] has to be implemented on
/// preferences that are to be included in Formatter preferences.
/// The trait may be implemented to indicate that the given preference has
/// a unicode key corresponding to it or be a custom one.
///
/// `ICU4X` provides an implementation of [`PreferenceKey`] for all
/// Unicode Extension Keys. The only external use of this trait is to implement
/// it on custom preferences that are to be included in a component preferences bag.
///
/// The below example show cases a manual generation of an `em` (emoji) unicode extension key
/// and a custom struct to showcase the difference in their behavior. For all use purposes,
/// the [`EmojiPresentationStyle`](crate::preferences::extensions::unicode::keywords::EmojiPresentationStyle) preference exposed by this crate should be used.
///
/// # Examples
/// ```
/// use icu::locale::{
///   extensions::unicode::{key, Key, value, Value},
///   preferences::{
///     define_preferences, PreferenceKey,
///     extensions::unicode::errors::PreferencesParseError,
///   },
/// };
///
/// #[non_exhaustive]
/// #[derive(Debug, Clone, Eq, PartialEq, Copy, Hash, Default)]
/// pub enum EmojiPresentationStyle {
///     Emoji,
///     Text,
///     #[default]
///     Default,
/// }
///
/// impl PreferenceKey for EmojiPresentationStyle {
///     fn unicode_extension_key() -> Option<Key> {
///         Some(key!("em"))
///     }
///
///     fn try_from_key_value(
///         key: &Key,
///         value: &Value,
///     ) -> Result<Option<Self>, PreferencesParseError> {
///         if Self::unicode_extension_key() == Some(*key) {
///             let subtag = value.as_single_subtag()
///                               .ok_or(PreferencesParseError::InvalidKeywordValue)?;
///             match subtag.as_str() {
///                 "emoji" => Ok(Some(Self::Emoji)),
///                 "text" => Ok(Some(Self::Text)),
///                 "default" => Ok(Some(Self::Default)),
///                 _ => Err(PreferencesParseError::InvalidKeywordValue)
///             }
///         } else {
///             Ok(None)
///         }
///     }
///
///     fn unicode_extension_value(&self) -> Option<Value> {
///         Some(match self {
///             EmojiPresentationStyle::Emoji => value!("emoji"),
///             EmojiPresentationStyle::Text => value!("text"),
///             EmojiPresentationStyle::Default => value!("default"),
///         })
///     }
/// }
///
/// #[non_exhaustive]
/// #[derive(Debug, Clone, Eq, PartialEq, Hash)]
/// pub struct CustomFormat {
///     value: String
/// }
///
/// impl PreferenceKey for CustomFormat {}
///
/// define_preferences!(
///     MyFormatterPreferences,
///     {
///         emoji: EmojiPresentationStyle,
///         custom: CustomFormat
///     }
/// );
/// ```
/// [`ICU4X`]: ../icu/index.html
pub trait PreferenceKey: Sized {
    /// Optional constructor of the given preference. It takes the
    /// unicode extension key and if the key matches it attemptes to construct
    /// the preference based on the given value.
    /// If the value is not a valid value for the given key, the constructor throws.
    fn try_from_key_value(
        _key: &crate::extensions::unicode::Key,
        _value: &crate::extensions::unicode::Value,
    ) -> Result<Option<Self>, crate::preferences::extensions::unicode::errors::PreferencesParseError>
    {
        Ok(None)
    }

    /// Retrieve unicode extension key corresponding to a given preference.
    fn unicode_extension_key() -> Option<crate::extensions::unicode::Key> {
        None
    }

    /// Retrieve unicode extension value corresponding to the given instance of the preference.
    fn unicode_extension_value(&self) -> Option<crate::extensions::unicode::Value> {
        None
    }
}

/// A macro to facilitate generation of preferences struct.
///
///
/// The generated preferences struct provides methods for merging and converting between [`Locale`] and
/// the preference bag. See [`preferences`](crate::preferences) for use cases.
///
/// In the example below, the input argument is the generated preferences struct which
/// can be auto-converted from a Locale, or combined from a Locale and Preferences Bag.
///
/// # Examples
/// ```
/// use icu::locale::{
///     preferences::{
///         define_preferences,
///         extensions::unicode::keywords::HourCycle
///     },
///     locale,
/// };
///
/// define_preferences!(
///     [Copy]
///     NoCalendarFormatterPreferences,
///     {
///         hour_cycle: HourCycle
///     }
/// );
///
/// struct NoCalendarFormatter {}
///
/// impl NoCalendarFormatter {
///     pub fn try_new(prefs: NoCalendarFormatterPreferences) -> Result<Self, ()> {
///         // load data and set struct fields based on the prefs input
///         Ok(Self {})
///     }
/// }
///
/// let loc = locale!("en-US");
///
/// let tf = NoCalendarFormatter::try_new(loc.into());
/// ```
///
/// [`Locale`]: crate::Locale
#[macro_export]
#[doc(hidden)]
macro_rules! __define_preferences {
    (
        $(#[$doc:meta])*
        $([$derive_attrs:ty])?
        $name:ident,
        {
            $(
                $(#[$key_doc:meta])*
                $key:ident: $pref:ty
            ),*
        }
     ) => (
        $(#[$doc])*
        #[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
        $(#[derive($derive_attrs)])?
        #[non_exhaustive]
        pub struct $name {
            /// Locale Preferences for the Preferences structure.
            pub locale_preferences: $crate::preferences::LocalePreferences,

            $(
                $(#[$key_doc])*
                pub $key: Option<$pref>,
            )*
        }

        impl From<$crate::Locale> for $name {
            fn from(loc: $crate::Locale) -> Self {
                $name::from(&loc)
            }
        }

        impl From<&$crate::Locale> for $name {
            fn from(loc: &$crate::Locale) -> Self {
                $name::from_locale_strict(loc).unwrap_or_else(|e| e)
            }
        }

        impl From<$crate::LanguageIdentifier> for $name {
            fn from(lid: $crate::LanguageIdentifier) -> Self {
                $name::from(&lid)
            }
        }

        impl From<&$crate::LanguageIdentifier> for $name {
            fn from(lid: &$crate::LanguageIdentifier) -> Self {
                Self {
                    locale_preferences: lid.into(),

                    $(
                        $key: None,
                    )*
                }
            }
        }

        // impl From<$name> for $crate::Locale {
        //     fn from(other: $name) -> Self {
        //         use $crate::preferences::PreferenceKey;
        //         let mut result = Self::from(other.locale_preferences);
        //         $(
        //             if let Some(value) = other.$key {
        //                 if let Some(ue) = <$pref>::unicode_extension_key() {
        //                     let val = value.unicode_extension_value().unwrap();
        //                     result.extensions.unicode.keywords.set(ue, val);
        //                 }
        //             }
        //         )*
        //         result
        //     }
        // }

        impl $name {
            /// Extends the preferences with the values from another set of preferences.
            pub fn extend(&mut self, other: $name) {
                self.locale_preferences.extend(other.locale_preferences);
                $(
                    if let Some(value) = other.$key {
                        self.$key = Some(value);
                    }
                )*
            }

            #[doc = concat!("Construct a `", stringify!($name), "` from a `Locale`")]
            ///
            /// Returns `Err` if any of of the preference values are invalid.
            pub fn from_locale_strict(loc: &$crate::Locale) -> Result<Self, Self> {
                use $crate::preferences::PreferenceKey;

                let mut is_err = false;

                $(
                    let mut $key = None;
                )*

                for (k, v) in loc.extensions.unicode.keywords.iter() {
                    $(

                        match <$pref>::try_from_key_value(k, v) {
                            Ok(Some(k)) => {
                                $key = Some(k);
                                continue;
                            }
                            Ok(None) => {}
                            Err(_) => {
                                is_err = true
                            }
                        }
                    )*
                }

                let r = Self {
                    locale_preferences: loc.into(),

                    $(
                        $key,
                    )*
                };

                if is_err {
                    Err(r)
                } else {
                    Ok(r)
                }
            }
        }
    )
}

#[macro_export]
#[doc(hidden)]
macro_rules! __prefs_convert {
    (
        $name1:ident,
        $name2:ident
    ) => {
        impl From<&$name1> for $name2 {
            fn from(other: &$name1) -> Self {
                let mut result = Self::default();
                result.locale_preferences = other.locale_preferences;
                result
            }
        }
    };
    (
        $name1:ident,
        $name2:ident,
        {
            $(
                $key:ident
            ),*
        }
    ) => {
        impl From<&$name1> for $name2 {
            fn from(other: &$name1) -> Self {
                let mut result = Self::default();
                result.locale_preferences = other.locale_preferences;
                $(
                    result.$key = other.$key;
                )*
                result
            }
        }
    };
}

#[doc(inline)]
pub use __define_preferences as define_preferences;

#[doc(inline)]
pub use __prefs_convert as prefs_convert;
