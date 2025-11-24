//! This example demonstrates:
//!
//! - The behavior of a derived `FromMeta` implementation for heterogeneous enums
//!   (i.e. enums that include a mix of unit, newtype and struct variants).
//! - Using `#[darling(word)]` to specify a unit variant to use when a receiver field
//!   is specified without a value (i.e. a unit variant to use for deriving the
//!   `FromMeta::from_word` method).
//! - Using `#[darling(default)]` on a receiver field to fall back to `Default::default()`
//!   for the enum's value when the receiver field is not specified by the caller.

use darling::{Error, FromDeriveInput, FromMeta};
use syn::parse_quote;

/// A playback volume.
#[derive(Debug, FromMeta, PartialEq, Eq)]
enum Volume {
    Normal,
    #[darling(word)]
    Low,
    High,
    #[darling(rename = "dB")]
    Decibels(u8),
}

impl Default for Volume {
    fn default() -> Self {
        Volume::Normal
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(play))]
struct PlayReceiver {
    #[darling(default)]
    volume: Volume,
}

fn main() {
    // `Default::default()` is used when `volume` is not specified.
    let missing_volume = PlayReceiver::from_derive_input(&parse_quote! {
        #[play]
        struct Player;
    })
    .unwrap();
    assert_eq!(Volume::Normal, missing_volume.volume);

    // `#[darling(word)]` unit variant is used when `volume` is specified as a word with no value.
    let empty_volume = PlayReceiver::from_derive_input(&parse_quote! {
        #[play(volume)]
        struct Player;
    })
    .unwrap();
    assert_eq!(Volume::Low, empty_volume.volume);

    // Specified `volume` value is used when provided.
    let unit_variant_volume = PlayReceiver::from_derive_input(&parse_quote! {
        #[play(volume(high))]
        struct Player;
    })
    .unwrap();
    assert_eq!(Volume::High, unit_variant_volume.volume);
    let newtype_volume = PlayReceiver::from_derive_input(&parse_quote! {
        #[play(volume(dB = 100))]
        struct Player;
    })
    .unwrap();
    assert_eq!(Volume::Decibels(100), newtype_volume.volume);

    // Multiple `volume` values result in an error.
    let err = PlayReceiver::from_derive_input(&parse_quote! {
        #[play(volume(low, dB = 20))]
        struct Player;
    })
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        Error::too_many_items(1).at("volume").to_string()
    );
}
