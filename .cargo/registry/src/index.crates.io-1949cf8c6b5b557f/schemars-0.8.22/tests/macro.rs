mod util;
use schemars::JsonSchema;
use util::*;

macro_rules! build_struct {
    (
        $id:ident { $($t:tt)* }
    ) => {
        #[allow(dead_code)]
        #[derive(JsonSchema)]
        pub struct $id {
            x: u8,
            $($t)*
        }
    };
}

build_struct!(A { v: i32 });

#[test]
fn macro_built_struct() -> TestResult {
    test_default_generated_schema::<A>("macro_built_struct")
}

macro_rules! build_enum {
    (
        $(#[$outer_derive:meta])*
        $outer:ident {
            $($(#[$inner_derive:meta])*
            $inner:ident {
                $( $(#[$field_attribute:meta])*
                   $field:ident : $ty:ty),*
            })*
        }
    ) => {

        $(
            $(#[$inner_derive])*
            pub struct $inner {
                $(
                    $(#[$field_attribute])*
                    pub $field: $ty
                ),*
            }
        )*

        $(#[$outer_derive])*
        pub enum $outer {
            $(
                $inner($inner)
            ),*
        }
    }
}

build_enum!(
    #[derive(JsonSchema)]
    OuterEnum {
        #[derive(JsonSchema)]
        InnerStruct {
            x: i32
        }
    }

);

#[test]
fn macro_built_enum() -> TestResult {
    test_default_generated_schema::<OuterEnum>("macro_built_enum")
}
