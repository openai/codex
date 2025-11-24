use darling::{
    util::{Override, SpannedValue},
    FromAttributes,
};

use quote::quote;
use syn::{parse::Parser, Attribute, Lit};

use Override::{Explicit, Inherit};

#[derive(FromAttributes, Default)]
#[darling(attributes(test_attr))]
struct SpannedValueOverPrimitive {
    string: SpannedValue<String>,
    optional_int: Option<SpannedValue<u64>>,
    override_literal: Override<SpannedValue<Lit>>,
    optional_override_char: Option<Override<SpannedValue<char>>>,
}

macro_rules! test_cases {
    ($(
        $name:ident : #[test_attr($($tok:tt)*)] => $result:ident $( {
            string: $string:expr,
            int: $int:expr,
            literal: $literal:pat,
            char: $char:expr,
        } )?;
    )*) => {$(
        #[test]
        fn $name() {
            let quoted = quote! {
                #[test_attr(
                   $($tok)*
                )]
            };

            let attrs = Attribute::parse_outer
                .parse2(quoted)
                .expect("failed to parse tokens as an attribute list");

            let result = SpannedValueOverPrimitive::from_attributes(&attrs);

            test_case_result!(result, $result $({
                string: $string,
                int: $int,
                literal: $literal,
                char: $char,
            })?);
        }
    )*}
}

macro_rules! test_case_result {
    ($value:expr, Err) => {
        assert!($value.is_err())
    };

    ($value:expr, Ok{
        string: $string:expr,
        int: $int:expr,
        literal: $literal:pat,
        char: $char:expr,
    }) => {{
        let out = $value.expect("failed to parse with from_attributes");

        assert_eq!(out.string.as_ref(), $string);
        assert_eq!(out.optional_int.map(|i| *i.as_ref()), $int);
        assert!(matches!(out.override_literal, $literal));
        assert_eq!(
            out.optional_override_char.map(|c| match c {
                Inherit => Inherit,
                Explicit(value) => Explicit(*value.as_ref()),
            }),
            $char
        )
    }};
}

test_cases! {
    basic: #[test_attr(
        string = "Hello",
        optional_int = 23,
        override_literal = "foo",
        optional_override_char = 'c'
    )] => Ok{
        string: "Hello",
        int: Some(23),
        literal: Explicit(_),
        char: Some(Override::Explicit('c')),
    };

    omit_optionals: #[test_attr(
        string = "Hello",
        override_literal = "foo",
    )] => Ok{
        string: "Hello",
        int: None,
        literal: Explicit(_),
        char: None,
    };

    omit_overrides: #[test_attr(
        string = "Hello",
        override_literal,
        optional_override_char,
    )] => Ok{
        string: "Hello",
        int: None,
        literal: Inherit,
        char: Some(Inherit),
    };
}
