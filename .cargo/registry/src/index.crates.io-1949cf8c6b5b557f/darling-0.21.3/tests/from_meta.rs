use darling::{Error, FromMeta};
use syn::parse_quote;

#[derive(Debug, FromMeta)]
#[darling(derive_syn_parse)]
struct Meta {
    #[darling(default)]
    meta1: Option<String>,
    #[darling(default)]
    meta2: bool,
}

#[test]
fn nested_meta_meta_value() {
    let meta = Meta::from_list(&[parse_quote! {
        meta1 = "thefeature"
    }])
    .unwrap();
    assert_eq!(meta.meta1, Some("thefeature".to_string()));
    assert!(!meta.meta2);
}

#[test]
fn nested_meta_meta_bool() {
    let meta = Meta::from_list(&[parse_quote! {
        meta2
    }])
    .unwrap();
    assert_eq!(meta.meta1, None);
    assert!(meta.meta2);
}

#[test]
fn nested_meta_lit_string_errors() {
    let err = Meta::from_list(&[parse_quote! {
        "meta2"
    }])
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        Error::unsupported_format("literal").to_string()
    );
}

#[test]
fn nested_meta_lit_integer_errors() {
    let err = Meta::from_list(&[parse_quote! {
        2
    }])
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        Error::unsupported_format("literal").to_string()
    );
}

#[test]
fn nested_meta_lit_bool_errors() {
    let err = Meta::from_list(&[parse_quote! {
        true
    }])
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        Error::unsupported_format("literal").to_string()
    );
}

#[test]
fn parse_impl() {
    let meta = parse_quote! {
        meta1 = "thefeature",
        meta2
    };
    let parsed_meta: Meta = syn::parse2(meta).unwrap();
    assert_eq!(parsed_meta.meta1, Some("thefeature".to_string()));
    assert!(parsed_meta.meta2);
}

/// Tests behavior of FromMeta implementation for enums.
mod enum_impl {
    use darling::{Error, FromMeta};
    use syn::parse_quote;

    /// A playback volume.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, FromMeta)]
    enum Volume {
        Normal,
        Low,
        High,
        #[darling(rename = "dB")]
        Decibels(u8),
    }

    #[test]
    fn string_for_unit_variant() {
        let volume = Volume::from_string("low").unwrap();
        assert_eq!(volume, Volume::Low);
    }

    #[test]
    fn single_value_list() {
        let unit_variant = Volume::from_list(&[parse_quote!(high)]).unwrap();
        assert_eq!(unit_variant, Volume::High);

        let newtype_variant = Volume::from_list(&[parse_quote!(dB = 100)]).unwrap();
        assert_eq!(newtype_variant, Volume::Decibels(100));
    }

    #[test]
    fn empty_list_errors() {
        let err = Volume::from_list(&[]).unwrap_err();
        assert_eq!(err.to_string(), Error::too_few_items(1).to_string());
    }

    #[test]
    fn multiple_values_list_errors() {
        let err = Volume::from_list(&[parse_quote!(low), parse_quote!(dB = 20)]).unwrap_err();
        assert_eq!(err.to_string(), Error::too_many_items(1).to_string());
    }
}

mod keyword {
    use darling::FromMeta;
    use quote::quote;
    use syn::{parse2, parse_quote, Path, Type};

    #[derive(Debug, FromMeta)]
    struct Keyword {
        #[darling(rename = "type")]
        ty: Type,
        #[darling(rename = "fn")]
        func: Path,
    }

    #[derive(Debug, FromMeta)]
    struct FlattenKeyword {
        #[darling(rename = "ref")]
        reference: Type,
        #[darling(flatten)]
        keyword: Keyword,
    }

    #[derive(Debug, PartialEq, Eq, FromMeta)]
    enum UnitEnumKeyword {
        #[darling(rename = "enum")]
        Enum,
        #[darling(rename = "struct")]
        Struct,
        #[darling(rename = "trait")]
        Trait,
    }

    #[derive(Debug, FromMeta)]
    struct FlattenEnumKeyword {
        #[darling(rename = "ref")]
        reference: Type,
        #[darling(flatten)]
        keyword: UnitEnumKeyword,
    }

    #[test]
    fn keywords() {
        let meta = quote! {
            outer(type = "u32", fn = foo)
        };

        let keyword = Keyword::from_meta(&parse2(meta).unwrap()).unwrap();
        assert_eq!(keyword.ty, parse_quote!(u32));
        assert_eq!(keyword.func, parse_quote!(foo));
    }

    #[test]
    fn flatten_keywords() {
        let meta = quote! {
            outer(ref = "u32", type = "i32", fn = bar)
        };

        let keyword = FlattenKeyword::from_meta(&parse2(meta).unwrap()).unwrap();
        assert_eq!(keyword.reference, parse_quote!(u32));
        assert_eq!(keyword.keyword.ty, parse_quote!(i32));
        assert_eq!(keyword.keyword.func, parse_quote!(bar));
    }

    #[test]
    fn enum_keywords() {
        let enum_ = quote! {
            outer(enum)
        };

        let unit_enum = UnitEnumKeyword::from_meta(&parse2(enum_).unwrap()).unwrap();
        assert_eq!(unit_enum, UnitEnumKeyword::Enum);

        let struct_ = quote! {
            outer(struct)
        };
        let unit_enum = UnitEnumKeyword::from_meta(&parse2(struct_).unwrap()).unwrap();
        assert_eq!(unit_enum, UnitEnumKeyword::Struct);
    }

    #[test]
    fn flatten_enum_keywords() {
        let meta = quote! {
            outer(ref = "u32", enum)
        };

        let keyword = FlattenEnumKeyword::from_meta(&parse2(meta).unwrap()).unwrap();
        assert_eq!(keyword.reference, parse_quote!(u32));
        assert_eq!(keyword.keyword, UnitEnumKeyword::Enum);

        let meta = quote! {
            outer(ref = "u32", struct)
        };

        let keyword = FlattenEnumKeyword::from_meta(&parse2(meta).unwrap()).unwrap();
        assert_eq!(keyword.reference, parse_quote!(u32));
        assert_eq!(keyword.keyword, UnitEnumKeyword::Struct);
    }
}

mod from_none_struct_closure {
    use darling::FromMeta;
    use syn::parse_quote;

    #[derive(Debug, FromMeta)]
    struct Outer {
        // Do NOT add `darling(default)` here; this is testing the `from_none` fallback
        // invoked when a field is not declared and no `default` is specified.
        speech: Example,
    }

    #[derive(Debug, FromMeta)]
    #[darling(from_none = || Some(Default::default()))]
    struct Example {
        max_volume: u32,
    }

    impl Default for Example {
        fn default() -> Self {
            Example { max_volume: 3 }
        }
    }

    #[test]
    fn absent_gets_from_none() {
        let thing = Outer::from_list(&[]).unwrap();
        assert_eq!(thing.speech.max_volume, 3);
    }

    #[test]
    fn word_errors() {
        let error = Outer::from_list(&[parse_quote!(speech)])
            .expect_err("speech should require its fields if declared");
        assert_eq!(error.len(), 1);
    }

    #[test]
    fn list_sets_field() {
        let thing = Outer::from_list(&[parse_quote!(speech(max_volume = 5))]).unwrap();
        assert_eq!(thing.speech.max_volume, 5);
    }
}

mod from_none_struct_path {
    use darling::FromMeta;
    use syn::parse_quote;

    #[derive(Debug, FromMeta)]
    struct Outer {
        // Do NOT add `darling(default)` here; this is testing the `from_none` fallback
        // invoked when a field is not declared and no `default` is specified.
        speech: Example,
    }

    fn from_none_fallback() -> Option<Example> {
        Some(Example { max_volume: 3 })
    }

    #[derive(Debug, FromMeta)]
    #[darling(from_none = from_none_fallback)]
    struct Example {
        max_volume: u32,
    }

    #[test]
    fn absent_gets_from_none() {
        let thing = Outer::from_list(&[]).unwrap();
        assert_eq!(thing.speech.max_volume, 3);
    }

    #[test]
    fn word_errors() {
        let error = Outer::from_list(&[parse_quote!(speech)])
            .expect_err("speech should require its fields if declared");
        assert_eq!(error.len(), 1);
    }

    #[test]
    fn list_sets_field() {
        let thing = Outer::from_list(&[parse_quote!(speech(max_volume = 5))]).unwrap();
        assert_eq!(thing.speech.max_volume, 5);
    }
}

mod from_word_struct_closure {
    use darling::FromMeta;
    use syn::parse_quote;

    #[derive(FromMeta)]
    struct Outer {
        #[darling(default)]
        speech: Example,
    }

    #[derive(FromMeta, Default)]
    #[darling(from_word = || Ok(Example { max_volume: 10 }))]
    struct Example {
        max_volume: u32,
    }

    #[test]
    fn absent_gets_default() {
        let thing = Outer::from_list(&[]).unwrap();
        assert_eq!(thing.speech.max_volume, 0);
    }

    #[test]
    fn word_gets_value() {
        let thing = Outer::from_list(&[parse_quote!(speech)]).unwrap();
        assert_eq!(thing.speech.max_volume, 10);
    }

    #[test]
    fn list_sets_field() {
        let thing = Outer::from_list(&[parse_quote!(speech(max_volume = 5))]).unwrap();
        assert_eq!(thing.speech.max_volume, 5);
    }
}

mod from_word_struct_path {
    use darling::FromMeta;
    use syn::parse_quote;

    #[derive(FromMeta)]
    struct Outer {
        #[darling(default)]
        speech: Example,
    }

    fn max_volume_10() -> darling::Result<Example> {
        Ok(Example { max_volume: 10 })
    }

    #[derive(FromMeta, Default)]
    #[darling(from_word = max_volume_10)]
    struct Example {
        max_volume: u32,
    }

    #[test]
    fn absent_gets_default() {
        let thing = Outer::from_list(&[]).unwrap();
        assert_eq!(thing.speech.max_volume, 0);
    }

    #[test]
    fn word_gets_value() {
        let thing = Outer::from_list(&[parse_quote!(speech)]).unwrap();
        assert_eq!(thing.speech.max_volume, 10);
    }

    #[test]
    fn list_sets_field() {
        let thing = Outer::from_list(&[parse_quote!(speech(max_volume = 5))]).unwrap();
        assert_eq!(thing.speech.max_volume, 5);
    }
}

mod from_word_enum_closure {
    use darling::FromMeta;
    use syn::parse_quote;

    #[derive(Debug, FromMeta)]
    struct Outer {
        speech: Example,
    }

    #[derive(Debug, FromMeta, PartialEq, Eq)]
    #[darling(from_word = || Ok(Example::Left { max_volume: 10 }))]
    enum Example {
        Left { max_volume: u32 },
        Right { speed: u32 },
    }

    #[test]
    fn word_gets_value() {
        let thing = Outer::from_list(&[parse_quote!(speech)]).unwrap();
        assert_eq!(thing.speech, Example::Left { max_volume: 10 });
    }

    #[test]
    fn list_sets_field() {
        let thing = Outer::from_list(&[parse_quote!(speech(left(max_volume = 5)))]).unwrap();
        assert_eq!(thing.speech, Example::Left { max_volume: 5 });
    }

    #[test]
    fn variant_word_fails() {
        let thing = Outer::from_list(&[parse_quote!(speech(left))]).expect_err(
            "A variant word is an error because from_word applies at the all-up enum level",
        );
        assert_eq!(thing.len(), 1);
    }
}
