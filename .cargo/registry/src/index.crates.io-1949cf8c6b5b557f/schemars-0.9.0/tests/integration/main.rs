#![allow(clippy::disallowed_names)]

#[cfg(feature = "arrayvec07")]
mod arrayvec;
mod bound;
#[cfg(feature = "bytes1")]
mod bytes;
#[cfg(feature = "chrono04")]
mod chrono;
mod contract;
mod crate_alias;
#[cfg(any(feature = "rust_decimal1", feature = "bigdecimal04"))]
mod decimal;
mod default;
mod deprecated;
mod docs;
#[cfg(feature = "either1")]
mod either;
mod enum_repr;
mod enums;
mod enums_deny_unknown_fields;
mod enums_flattened;
mod examples;
mod extend;
mod flatten;
mod from_value;
mod garde;
#[cfg(feature = "indexmap2")]
mod indexmap;
mod inline_subschemas;
#[cfg(feature = "jiff02")]
mod jiff;
mod macros;
mod remote_derive;
mod same_name;
mod schema_name;
mod schema_with;
#[cfg(feature = "semver1")]
mod semver;
mod settings;
mod skip;
#[cfg(feature = "smallvec1")]
mod smallvec;
#[cfg(feature = "smol_str02")]
mod smol_str;
mod std_types;
mod structs;
mod transform;
mod transparent;
#[cfg(feature = "url2")]
mod url;
#[cfg(feature = "uuid1")]
mod uuid;
mod validator;

mod prelude {
    pub(crate) use crate::test;
    pub(crate) use crate::test_helper::{arbitrary_values, arbitrary_values_except};
    pub(crate) use schemars::JsonSchema;
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{json, Value};
}

mod test_helper;

#[macro_export]
macro_rules! test_name {
    () => {{
        fn f() {}
        fn type_name_of_val<T>(_: T) -> &'static str {
            core::any::type_name::<T>()
        }
        let test_fn_name = type_name_of_val(f)
            .trim_end_matches("::f")
            .split("::")
            .last()
            .unwrap();
        format!("{}~{}", core::file!(), test_fn_name)
    }};
}

#[macro_export]
macro_rules! test {
    ($type:ty, $settings:expr) => {
        $crate::test_helper::TestHelper::<$type>::new($crate::test_name!(), $settings)
    };
    ($type:ty) => {
        test!($type, schemars::generate::SchemaSettings::default())
    };
    (value: $value:expr, $settings:expr) => {
        $crate::test_helper::TestHelper::new_for_value($crate::test_name!(), $settings, $value)
    };
    (value: $value:expr) => {
        test!(value: $value, schemars::generate::SchemaSettings::default())
    };
}
