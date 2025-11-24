#![deny(warnings)]
/// Provides demodulize a string.
///
/// Example string `Foo::Bar` becomes `Bar`
#[cfg(feature = "heavyweight")]
pub mod demodulize;
/// Provides deconstantizea string.
///
/// Example string `Foo::Bar` becomes `Foo`
#[cfg(feature = "heavyweight")]
pub mod deconstantize;
/// Provides conversion to plural strings.
///
/// Example string `FooBar` -> `FooBars`
#[cfg(feature = "heavyweight")]
pub mod pluralize;
/// Provides conversion to singular strings.
///
/// Example string `FooBars` -> `FooBar`
#[cfg(feature = "heavyweight")]
pub mod singularize;

mod constants;
