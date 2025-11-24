use proc_macro2;

use ast;
use attr;
use bound;
use syn;

/// Make generic with all the generics in the input, plus a bound `T: <trait_path>` for each
/// generic field type that will be shown.
pub fn build_impl_generics<F, G, H>(
    item: &ast::Input,
    trait_path: &syn::Path,
    needs_debug_bound: F,
    field_bound: G,
    input_bound: H,
) -> syn::Generics
where
    F: Fn(&attr::Field) -> bool,
    G: Fn(&attr::Field) -> Option<&[syn::WherePredicate]>,
    H: Fn(&attr::Input) -> Option<&[syn::WherePredicate]>,
{
    let generics = bound::without_defaults(item.generics);
    let generics = bound::with_where_predicates_from_fields(item, &generics, field_bound);

    match input_bound(&item.attrs) {
        Some(predicates) => bound::with_where_predicates(&generics, predicates),
        None => bound::with_bound(item, &generics, needs_debug_bound, trait_path),
    }
}

/// Construct a name for the inner type parameter that can't collide with any
/// type parameters of the item. This is achieved by starting with a base and
/// then concatenating the names of all other type parameters.
pub fn hygienic_type_parameter(item: &ast::Input, base: &str) -> syn::Ident {
    let mut typaram = String::with_capacity(150);
    typaram.push_str(base);
    let typaram = item.generics.type_params().fold(typaram, |mut acc, ty| {
        acc.push_str(&format!("{}", &ty.ident));
        acc
    });

    syn::Ident::new(&typaram, proc_macro2::Span::call_site())
}
