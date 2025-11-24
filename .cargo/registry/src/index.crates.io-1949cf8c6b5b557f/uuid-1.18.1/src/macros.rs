macro_rules! define_uuid_macro {
    {$(#[$doc:meta])*} => {
        $(#[$doc])*
        #[cfg(feature = "macro-diagnostics")]
        #[macro_export]
        macro_rules! uuid {
            ($uuid:expr) => {{
                const OUTPUT: $crate::Uuid = match $crate::Uuid::try_parse($uuid) {
                    $crate::__macro_support::Ok(u) => u,
                    $crate::__macro_support::Err(_) => panic!("invalid UUID"),
                };
                OUTPUT
            }};
            ($uuid:literal) => {{
                $crate::Uuid::from_bytes($crate::uuid_macro_internal::parse_lit!($uuid))
            }};
        }

        $(#[$doc])*
        #[cfg(not(feature = "macro-diagnostics"))]
        #[macro_export]
        macro_rules! uuid {
            ($uuid:expr) => {{
                const OUTPUT: $crate::Uuid = match $crate::Uuid::try_parse($uuid) {
                    $crate::__macro_support::Ok(u) => u,
                    $crate::__macro_support::Err(_) => panic!("invalid UUID"),
                };
                OUTPUT
            }};
        }
    }
}

define_uuid_macro! {
/// Parse [`Uuid`][uuid::Uuid]s from string literals at compile time.
///
/// ## Usage
///
/// This macro transforms the string literal representation of a
/// [`Uuid`][uuid::Uuid] into the bytes representation, raising a compilation
/// error if it cannot properly be parsed.
///
/// ## Examples
///
/// Setting a global constant:
///
/// ```
/// # use uuid::{uuid, Uuid};
/// pub const SCHEMA_ATTR_CLASS: Uuid = uuid!("00000000-0000-0000-0000-ffff00000000");
/// pub const SCHEMA_ATTR_UUID: Uuid = uuid!("00000000-0000-0000-0000-ffff00000001");
/// pub const SCHEMA_ATTR_NAME: Uuid = uuid!("00000000-0000-0000-0000-ffff00000002");
/// ```
///
/// Defining a local variable:
///
/// ```
/// # use uuid::uuid;
/// let uuid = uuid!("urn:uuid:F9168C5E-CEB2-4faa-B6BF-329BF39FA1E4");
/// ```
/// Using a const variable:
/// ```
/// # use uuid::uuid;
/// const UUID_STR: &str = "12345678-1234-5678-1234-567812345678";
/// let UUID = uuid!(UUID_STR);
/// ```
///
/// ## Compilation Failures
///
/// Invalid UUIDs are rejected:
///
/// ```compile_fail
/// # use uuid::uuid;
/// let uuid = uuid!("F9168C5E-ZEB2-4FAA-B6BF-329BF39FA1E4");
/// ```
///
/// Enable the feature `macro-diagnostics` to see the error messages below.
///
/// Provides the following compilation error:
///
/// ```txt
/// error: invalid character: expected an optional prefix of `urn:uuid:` followed by [0-9a-fA-F-], found Z at 9
///     |
///     |     let id = uuid!("F9168C5E-ZEB2-4FAA-B6BF-329BF39FA1E4");
///     |                              ^
/// ```
///
/// [uuid::Uuid]: https://docs.rs/uuid/*/uuid/struct.Uuid.html
}

// Internal macros

// These `transmute` macros are a stepping stone towards `zerocopy` integration.
// When the `zerocopy` feature is enabled, which it is in CI, the transmutes are 
// checked by it

// SAFETY: Callers must ensure this call would be safe when handled by zerocopy
#[cfg(not(all(uuid_unstable, feature = "zerocopy")))]
macro_rules! unsafe_transmute_ref(
    ($e:expr) => { unsafe { core::mem::transmute::<&_, &_>($e) } }
);

// SAFETY: Callers must ensure this call would be safe when handled by zerocopy
#[cfg(all(uuid_unstable, feature = "zerocopy"))]
macro_rules! unsafe_transmute_ref(
    ($e:expr) => { zerocopy::transmute_ref!($e) }
);

// SAFETY: Callers must ensure this call would be safe when handled by zerocopy
#[cfg(not(all(uuid_unstable, feature = "zerocopy")))]
macro_rules! unsafe_transmute(
    ($e:expr) => { unsafe { core::mem::transmute::<_, _>($e) } }
);

// SAFETY: Callers must ensure this call would be safe when handled by zerocopy
#[cfg(all(uuid_unstable, feature = "zerocopy"))]
macro_rules! unsafe_transmute(
    ($e:expr) => { zerocopy::transmute!($e) }
);
