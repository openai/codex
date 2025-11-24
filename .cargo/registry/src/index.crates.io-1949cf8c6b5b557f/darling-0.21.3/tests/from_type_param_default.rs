use darling::FromTypeParam;
use syn::{parse_quote, DeriveInput, GenericParam, TypeParam};

#[derive(Default, FromTypeParam)]
#[darling(attributes(lorem), default)]
struct Lorem {
    foo: bool,
    bar: Option<String>,
    default: Option<syn::Type>,
}

fn extract_type(param: &GenericParam) -> &TypeParam {
    match *param {
        GenericParam::Type(ref ty) => ty,
        _ => unreachable!("Not a type param"),
    }
}

#[test]
#[allow(clippy::bool_assert_comparison)]
fn expand_many() {
    let di: DeriveInput = parse_quote! {
        struct Baz<
            #[lorem(foo)] T,
            #[lorem(bar = "x")] U: Eq + ?Sized,
            #[lorem(foo = false)] V = (),
        >(T, U, V);
    };
    let params = di.generics.params;

    {
        let ty = extract_type(&params[0]);
        let lorem = Lorem::from_type_param(ty).unwrap();
        assert_eq!(lorem.foo, true);
        assert_eq!(lorem.bar, None);
    }

    {
        let ty = extract_type(&params[1]);
        let lorem = Lorem::from_type_param(ty).unwrap();
        assert_eq!(lorem.foo, false);
        assert_eq!(lorem.bar, Some("x".to_string()));
        assert!(lorem.default.is_none());
    }

    {
        let ty = extract_type(&params[2]);
        let lorem = Lorem::from_type_param(ty).unwrap();
        assert_eq!(lorem.foo, false);
        assert_eq!(lorem.bar, None);
        assert!(lorem.default.is_some());
    }
}
