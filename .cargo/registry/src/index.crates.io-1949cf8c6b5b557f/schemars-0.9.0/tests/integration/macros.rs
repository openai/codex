use crate::prelude::*;

macro_rules! build_struct {
    (
        $id:ident { $($t:tt)* }
    ) => {
        #[derive(JsonSchema, Deserialize, Serialize, Default)]
        pub struct $id {
            x: u8,
            $($t)*
        }
    };
}

build_struct!(A { v: i32 });

#[test]
fn macro_built_struct() {
    test!(A).assert_allows_ser_roundtrip_default();
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
    #[derive(JsonSchema, Deserialize, Serialize)]
    OuterEnum {
        #[derive(JsonSchema, Deserialize, Serialize, Default)]
        InnerStruct {
            x: i32
        }
    }

);

#[test]
fn macro_built_enum() {
    test!(OuterEnum).assert_allows_ser_roundtrip([OuterEnum::InnerStruct(InnerStruct::default())]);
}
