use syn::punctuated::Punctuated;
use syn::{Ident, Type};

use crate::usage::{IdentRefSet, IdentSet, Options};

/// Searcher for finding type params in a syntax tree.
/// This can be used to determine if a given type parameter needs to be bounded in a generated impl.
pub trait UsesTypeParams {
    /// Returns the subset of the queried type parameters that are used by the implementing syntax element.
    ///
    /// This method only accounts for direct usage by the element; indirect usage via bounds or `where`
    /// predicates are not detected.
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a>;

    /// Find all type params using `uses_type_params`, then clone the found values and return the set.
    fn uses_type_params_cloned(&self, options: &Options, type_set: &IdentSet) -> IdentSet {
        self.uses_type_params(options, type_set)
            .into_iter()
            .cloned()
            .collect()
    }
}

/// Searcher for finding type params in an iterator.
///
/// This trait extends iterators, providing a way to turn a filtered list of fields or variants into a set
/// of type parameter idents.
pub trait CollectTypeParams {
    /// Consume an iterator, accumulating all type parameters in the elements which occur in `type_set`.
    fn collect_type_params<'a>(self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a>;

    /// Consume an iterator using `collect_type_params`, then clone all found type params and return that set.
    fn collect_type_params_cloned(self, options: &Options, type_set: &IdentSet) -> IdentSet;
}

impl<'i, T, I> CollectTypeParams for T
where
    T: IntoIterator<Item = &'i I>,
    I: 'i + UsesTypeParams,
{
    fn collect_type_params<'a>(self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        self.into_iter().fold(
            IdentRefSet::with_capacity_and_hasher(type_set.len(), Default::default()),
            |state, value| union_in_place(state, value.uses_type_params(options, type_set)),
        )
    }

    fn collect_type_params_cloned(self, options: &Options, type_set: &IdentSet) -> IdentSet {
        self.collect_type_params(options, type_set)
            .into_iter()
            .cloned()
            .collect()
    }
}

/// Insert the contents of `right` into `left`.
fn union_in_place<'a>(mut left: IdentRefSet<'a>, right: IdentRefSet<'a>) -> IdentRefSet<'a> {
    left.extend(right);

    left
}

impl UsesTypeParams for () {
    fn uses_type_params<'a>(&self, _options: &Options, _type_set: &'a IdentSet) -> IdentRefSet<'a> {
        Default::default()
    }
}

impl<T: UsesTypeParams> UsesTypeParams for Option<T> {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        self.as_ref()
            .map(|v| v.uses_type_params(options, type_set))
            .unwrap_or_default()
    }
}

impl<T: UsesTypeParams> UsesTypeParams for Vec<T> {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        self.collect_type_params(options, type_set)
    }
}

impl<T: UsesTypeParams, U> UsesTypeParams for Punctuated<T, U> {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        self.collect_type_params(options, type_set)
    }
}

uses_type_params!(syn::AngleBracketedGenericArguments, args);
uses_type_params!(syn::AssocType, ty);
uses_type_params!(syn::BareFnArg, ty);
uses_type_params!(syn::Constraint, bounds);
uses_type_params!(syn::DataEnum, variants);
uses_type_params!(syn::DataStruct, fields);
uses_type_params!(syn::DataUnion, fields);
uses_type_params!(syn::Field, ty);
uses_type_params!(syn::FieldsNamed, named);
uses_type_params!(syn::ParenthesizedGenericArguments, inputs, output);
uses_type_params!(syn::PredicateType, bounded_ty, bounds);
uses_type_params!(syn::QSelf, ty);
uses_type_params!(syn::TraitBound, path);
uses_type_params!(syn::TypeArray, elem);
uses_type_params!(syn::TypeBareFn, inputs, output);
uses_type_params!(syn::TypeGroup, elem);
uses_type_params!(syn::TypeImplTrait, bounds);
uses_type_params!(syn::TypeParen, elem);
uses_type_params!(syn::TypePtr, elem);
uses_type_params!(syn::TypeReference, elem);
uses_type_params!(syn::TypeSlice, elem);
uses_type_params!(syn::TypeTuple, elems);
uses_type_params!(syn::TypeTraitObject, bounds);
uses_type_params!(syn::Variant, fields);

impl UsesTypeParams for syn::Data {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        match *self {
            syn::Data::Struct(ref v) => v.uses_type_params(options, type_set),
            syn::Data::Enum(ref v) => v.uses_type_params(options, type_set),
            syn::Data::Union(ref v) => v.uses_type_params(options, type_set),
        }
    }
}

impl UsesTypeParams for syn::Fields {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        self.collect_type_params(options, type_set)
    }
}

/// Check if an Ident exactly matches one of the sought-after type parameters.
impl UsesTypeParams for Ident {
    fn uses_type_params<'a>(&self, _options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        type_set.iter().filter(|v| *v == self).collect()
    }
}

impl UsesTypeParams for syn::ReturnType {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        if let syn::ReturnType::Type(_, ref ty) = *self {
            ty.uses_type_params(options, type_set)
        } else {
            Default::default()
        }
    }
}

impl UsesTypeParams for Type {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        match *self {
            Type::Slice(ref v) => v.uses_type_params(options, type_set),
            Type::Array(ref v) => v.uses_type_params(options, type_set),
            Type::Ptr(ref v) => v.uses_type_params(options, type_set),
            Type::Reference(ref v) => v.uses_type_params(options, type_set),
            Type::BareFn(ref v) => v.uses_type_params(options, type_set),
            Type::Tuple(ref v) => v.uses_type_params(options, type_set),
            Type::Path(ref v) => v.uses_type_params(options, type_set),
            Type::Paren(ref v) => v.uses_type_params(options, type_set),
            Type::Group(ref v) => v.uses_type_params(options, type_set),
            Type::TraitObject(ref v) => v.uses_type_params(options, type_set),
            Type::ImplTrait(ref v) => v.uses_type_params(options, type_set),
            Type::Macro(_) | Type::Verbatim(_) | Type::Infer(_) | Type::Never(_) => {
                Default::default()
            }
            _ => panic!("Unknown syn::Type: {:?}", self),
        }
    }
}

impl UsesTypeParams for syn::TypePath {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        let hits = self.path.uses_type_params(options, type_set);

        if options.include_type_path_qself() {
            union_in_place(hits, self.qself.uses_type_params(options, type_set))
        } else {
            hits
        }
    }
}

impl UsesTypeParams for syn::Path {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        // Not sure if this is even possible, but a path with no segments definitely
        // can't use type parameters.
        if self.segments.is_empty() {
            return Default::default();
        }

        // A path segment ident can only match if it is not global and it is the first segment
        // in the path.
        let ident_hits = if self.leading_colon.is_none() {
            self.segments[0].ident.uses_type_params(options, type_set)
        } else {
            Default::default()
        };

        // Merge ident hit, if any, with all hits from path arguments
        self.segments.iter().fold(ident_hits, |state, segment| {
            union_in_place(state, segment.arguments.uses_type_params(options, type_set))
        })
    }
}

impl UsesTypeParams for syn::PathArguments {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        match *self {
            syn::PathArguments::None => Default::default(),
            syn::PathArguments::AngleBracketed(ref v) => v.uses_type_params(options, type_set),
            syn::PathArguments::Parenthesized(ref v) => v.uses_type_params(options, type_set),
        }
    }
}

impl UsesTypeParams for syn::WherePredicate {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        match *self {
            syn::WherePredicate::Lifetime(_) => Default::default(),
            syn::WherePredicate::Type(ref v) => v.uses_type_params(options, type_set),
            // non-exhaustive enum
            // TODO: replace panic with failible function
            _ => panic!("Unknown syn::WherePredicate: {:?}", self),
        }
    }
}

impl UsesTypeParams for syn::GenericArgument {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        match *self {
            syn::GenericArgument::Type(ref v) => v.uses_type_params(options, type_set),
            syn::GenericArgument::AssocType(ref v) => v.uses_type_params(options, type_set),
            syn::GenericArgument::Constraint(ref v) => v.uses_type_params(options, type_set),
            syn::GenericArgument::AssocConst(_)
            | syn::GenericArgument::Const(_)
            | syn::GenericArgument::Lifetime(_) => Default::default(),
            // non-exhaustive enum
            // TODO: replace panic with failible function
            _ => panic!("Unknown syn::GenericArgument: {:?}", self),
        }
    }
}

impl UsesTypeParams for syn::TypeParamBound {
    fn uses_type_params<'a>(&self, options: &Options, type_set: &'a IdentSet) -> IdentRefSet<'a> {
        match *self {
            syn::TypeParamBound::Trait(ref v) => v.uses_type_params(options, type_set),
            syn::TypeParamBound::Lifetime(_) => Default::default(),
            // non-exhaustive enum
            // TODO: replace panic with failible function
            _ => panic!("Unknown syn::TypeParamBound: {:?}", self),
        }
    }
}

#[cfg(test)]
mod tests {
    use proc_macro2::Span;
    use syn::{parse_quote, DeriveInput, Ident};

    use super::UsesTypeParams;
    use crate::usage::IdentSet;
    use crate::usage::Purpose::*;

    fn ident_set(idents: Vec<&str>) -> IdentSet {
        idents
            .into_iter()
            .map(|s| Ident::new(s, Span::call_site()))
            .collect()
    }

    #[test]
    fn finds_simple() {
        let input: DeriveInput = parse_quote! { struct Foo<T, U>(T, i32, A, U); };
        let generics = ident_set(vec!["T", "U", "X"]);
        let matches = input.data.uses_type_params(&BoundImpl.into(), &generics);
        assert_eq!(matches.len(), 2);
        assert!(matches.contains::<Ident>(&parse_quote!(T)));
        assert!(matches.contains::<Ident>(&parse_quote!(U)));
        assert!(!matches.contains::<Ident>(&parse_quote!(X)));
        assert!(!matches.contains::<Ident>(&parse_quote!(A)));
    }

    #[test]
    fn finds_named() {
        let input: DeriveInput = parse_quote! {
            struct Foo<T, U = usize> {
                bar: T,
                world: U,
            }
        };

        let generics = ident_set(vec!["T", "U", "X"]);

        let matches = input.data.uses_type_params(&BoundImpl.into(), &generics);

        assert_eq!(matches.len(), 2);
        assert!(matches.contains::<Ident>(&parse_quote!(T)));
        assert!(matches.contains::<Ident>(&parse_quote!(U)));
        assert!(!matches.contains::<Ident>(&parse_quote!(X)));
        assert!(!matches.contains::<Ident>(&parse_quote!(A)));
    }

    #[test]
    fn finds_as_type_arg() {
        let input: DeriveInput = parse_quote! {
            struct Foo<T, U> {
                bar: T,
                world: Vec<U>,
            }
        };

        let generics = ident_set(vec!["T", "U", "X"]);

        let matches = input.data.uses_type_params(&BoundImpl.into(), &generics);

        assert_eq!(matches.len(), 2);
        assert!(matches.contains::<Ident>(&parse_quote!(T)));
        assert!(matches.contains::<Ident>(&parse_quote!(U)));
        assert!(!matches.contains::<Ident>(&parse_quote!(X)));
        assert!(!matches.contains::<Ident>(&parse_quote!(A)));
    }

    #[test]
    fn associated_type() {
        let input: DeriveInput =
            parse_quote! { struct Foo<'a, T> where T: Iterator { peek: T::Item } };
        let generics = ident_set(vec!["T", "INTO"]);
        let matches = input.data.uses_type_params(&BoundImpl.into(), &generics);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn box_fn_output() {
        let input: DeriveInput = parse_quote! { struct Foo<T>(Box<dyn Fn() -> T>); };
        let generics = ident_set(vec!["T"]);
        let matches = input.data.uses_type_params(&BoundImpl.into(), &generics);
        assert_eq!(matches.len(), 1);
        assert!(matches.contains::<Ident>(&parse_quote!(T)));
    }

    #[test]
    fn box_fn_input() {
        let input: DeriveInput = parse_quote! { struct Foo<T>(Box<dyn Fn(&T) -> ()>); };
        let generics = ident_set(vec!["T"]);
        let matches = input.data.uses_type_params(&BoundImpl.into(), &generics);
        assert_eq!(matches.len(), 1);
        assert!(matches.contains::<Ident>(&parse_quote!(T)));
    }

    /// Test that `syn::TypePath` is correctly honoring the different modes a
    /// search can execute in.
    #[test]
    fn qself_vec() {
        let input: DeriveInput =
            parse_quote! { struct Foo<T>(<Vec<T> as a::b::Trait>::AssociatedItem); };
        let generics = ident_set(vec!["T", "U"]);

        let bound_matches = input.data.uses_type_params(&BoundImpl.into(), &generics);
        assert_eq!(bound_matches.len(), 0);

        let declare_matches = input.data.uses_type_params(&Declare.into(), &generics);
        assert_eq!(declare_matches.len(), 1);
        assert!(declare_matches.contains::<Ident>(&parse_quote!(T)));
    }
}
