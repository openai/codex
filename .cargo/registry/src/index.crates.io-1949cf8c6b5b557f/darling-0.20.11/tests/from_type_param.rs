use darling::FromTypeParam;
use syn::{parse_quote, DeriveInput, GenericParam, Ident, TypeParam};

#[derive(FromTypeParam)]
#[darling(attributes(lorem), from_ident)]
struct Lorem {
    ident: Ident,
    bounds: Vec<syn::TypeParamBound>,
    foo: bool,
    bar: Option<String>,
}

impl From<Ident> for Lorem {
    fn from(ident: Ident) -> Self {
        Lorem {
            ident,
            foo: false,
            bar: None,
            bounds: Default::default(),
        }
    }
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
            #[lorem(bar = "x")] U: Eq + ?Sized
        >(T, U);
    };

    let params = di.generics.params;

    {
        let ty = extract_type(&params[0]);
        let lorem = Lorem::from_type_param(ty).unwrap();
        assert_eq!(lorem.ident, "T");
        assert_eq!(lorem.foo, true);
        assert_eq!(lorem.bar, None);
    }

    {
        let ty = extract_type(&params[1]);
        let lorem = Lorem::from_type_param(ty).unwrap();
        assert_eq!(lorem.ident, "U");
        assert_eq!(lorem.foo, false);
        assert_eq!(lorem.bar, Some("x".to_string()));
        assert_eq!(lorem.bounds.len(), 2);
    }
}
