//! Consumer-owned compatibility contracts for independently deployed executors.

mod manifest;

pub use manifest::ContractManifest;
pub use manifest::ContractManifestError;
pub use manifest::ContractTest;

#[doc(hidden)]
pub use inventory as __inventory;

/// Declares and registers a complete behavioral contract test file.
#[macro_export]
macro_rules! behavioral_contract {
    ($name:ident, $source:literal $(,)?) => {
        $crate::__inventory::submit! {
            $crate::ContractTest::new(stringify!($name), include_str!($source))
        }

        #[path = $source]
        mod $name;
    };
}
