use darling::{util::Flag, FromDeriveInput, FromMeta};
use proc_macro2::Ident;
use syn::parse_quote;

#[derive(FromMeta)]
struct Vis {
    public: Flag,
    private: Flag,
}

#[derive(FromDeriveInput)]
#[darling(attributes(sample))]
struct Example {
    ident: Ident,
    label: String,
    #[darling(flatten)]
    visibility: Vis,
}

#[test]
fn happy_path() {
    let di = Example::from_derive_input(&parse_quote! {
        #[sample(label = "Hello", public)]
        struct Demo {}
    });

    let parsed = di.unwrap();
    assert_eq!(parsed.ident, "Demo");
    assert_eq!(&parsed.label, "Hello");
    assert!(parsed.visibility.public.is_present());
    assert!(!parsed.visibility.private.is_present());
}

#[test]
fn unknown_field_errors() {
    let errors = Example::from_derive_input(&parse_quote! {
        #[sample(label = "Hello", republic)]
        struct Demo {}
    })
    .map(|_| "Should have failed")
    .unwrap_err();

    assert_eq!(errors.len(), 1);
}

/// This test demonstrates flatten being used recursively.
/// Fields are expected to be consumed by the outermost matching struct.
#[test]
fn recursive_flattening() {
    #[derive(FromMeta)]
    struct Nested2 {
        above: isize,
        below: isize,
        port: Option<isize>,
    }

    #[derive(FromMeta)]
    struct Nested1 {
        port: isize,
        starboard: isize,
        #[darling(flatten)]
        z_axis: Nested2,
    }

    #[derive(FromMeta)]
    struct Nested0 {
        fore: isize,
        aft: isize,
        #[darling(flatten)]
        cross_section: Nested1,
    }

    #[derive(FromDeriveInput)]
    #[darling(attributes(boat))]
    struct BoatPosition {
        #[darling(flatten)]
        pos: Nested0,
    }

    let parsed = BoatPosition::from_derive_input(&parse_quote! {
        #[boat(fore = 1, aft = 1, port = 10, starboard = 50, above = 20, below = -3)]
        struct Demo;
    })
    .unwrap();

    assert_eq!(parsed.pos.fore, 1);
    assert_eq!(parsed.pos.aft, 1);

    assert_eq!(parsed.pos.cross_section.port, 10);
    assert_eq!(parsed.pos.cross_section.starboard, 50);

    assert_eq!(parsed.pos.cross_section.z_axis.above, 20);
    assert_eq!(parsed.pos.cross_section.z_axis.below, -3);
    // This should be `None` because the `port` field in `Nested1` consumed
    // the field before the leftovers were passed to `Nested2::from_list`.
    assert_eq!(parsed.pos.cross_section.z_axis.port, None);
}

/// This test confirms that a collection - in this case a HashMap - can
/// be used with `flatten`.
#[test]
fn flattening_into_hashmap() {
    #[derive(FromDeriveInput)]
    #[darling(attributes(ca))]
    struct Catchall {
        hello: String,
        volume: usize,
        #[darling(flatten)]
        others: std::collections::HashMap<String, String>,
    }

    let parsed = Catchall::from_derive_input(&parse_quote! {
        #[ca(hello = "World", volume = 10, first_name = "Alice", second_name = "Bob")]
        struct Demo;
    })
    .unwrap();

    assert_eq!(parsed.hello, "World");
    assert_eq!(parsed.volume, 10);
    assert_eq!(parsed.others.len(), 2);
}

#[derive(FromMeta)]
#[allow(dead_code)]
struct Person {
    first: String,
    last: String,
    parent: Option<Box<Person>>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(v))]
#[allow(dead_code)]
struct Outer {
    #[darling(flatten)]
    owner: Person,
    #[darling(default)]
    blast: bool,
}

/// This test makes sure that field names from parent structs are not inappropriately
/// offered as alternates for unknown field errors in child structs.
///
/// A naive implementation that tried to offer all the flattened fields for "did you mean"
/// could inspect all errors returned by the flattened field's `from_list` call and add the
/// parent's field names as alternates to all unknown field errors.
///
/// THIS WOULD BE INCORRECT. Those unknown field errors may have already come from
/// child fields within the flattened struct, where the parent's field names are not valid.
#[test]
fn do_not_suggest_invalid_alts() {
    let errors = Outer::from_derive_input(&parse_quote! {
        #[v(first = "Hello", last = "World", parent(first = "Hi", last = "Earth", blasts = "off"))]
        struct Demo;
    })
    .map(|_| "Should have failed")
    .unwrap_err()
    .to_string();

    assert!(
        !errors.contains("`blast`"),
        "Should not contain `blast`: {}",
        errors
    );
}

#[test]
#[cfg(feature = "suggestions")]
fn suggest_valid_parent_alts() {
    let errors = Outer::from_derive_input(&parse_quote! {
        #[v(first = "Hello", bladt = false, last = "World", parent(first = "Hi", last = "Earth"))]
        struct Demo;
    })
    .map(|_| "Should have failed")
    .unwrap_err()
    .to_string();
    assert!(
        errors.contains("`blast`"),
        "Should contain `blast` as did-you-mean suggestion: {}",
        errors
    );
}

/// Make sure that flatten works with smart pointer types, e.g. `Box`.
///
/// The generated `flatten` impl directly calls `FromMeta::from_list`
/// rather than calling `from_meta`, and the default impl of `from_list`
/// will return an unsupported format error; this test ensures that the
/// smart pointer type is properly forwarding the `from_list` call.
#[test]
fn flattening_to_box() {
    #[derive(FromDeriveInput)]
    #[darling(attributes(v))]
    struct Example {
        #[darling(flatten)]
        items: Box<Vis>,
    }

    let when_omitted = Example::from_derive_input(&parse_quote! {
        struct Demo;
    })
    .unwrap();

    assert!(!when_omitted.items.public.is_present());
}
