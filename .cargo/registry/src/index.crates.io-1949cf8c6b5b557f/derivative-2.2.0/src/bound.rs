/* This file incorporates work covered by the following copyright and
 * permission notice:
 *   Copyright 2016 The serde Developers. See
 *   https://github.com/serde-rs/serde/blob/3f28a9324042950afa80354722aeeee1a55cbfa3/README.md#license.
 *
 *   Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 *   http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 *   <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 *   option. This file may not be copied, modified, or distributed
 *   except according to those terms.
 */

use ast;
use attr;
use std::collections::HashSet;
use syn::{self, visit, GenericParam};

// use internals::ast::Item;
// use internals::attr;

/// Remove the default from every type parameter because in the generated `impl`s
/// they look like associated types: "error: associated type bindings are not
/// allowed here".
pub fn without_defaults(generics: &syn::Generics) -> syn::Generics {
    syn::Generics {
        params: generics
            .params
            .iter()
            .map(|generic_param| match *generic_param {
                GenericParam::Type(ref ty_param) => syn::GenericParam::Type(syn::TypeParam {
                    default: None,
                    ..ty_param.clone()
                }),
                ref param => param.clone(),
            })
            .collect(),
        ..generics.clone()
    }
}

pub fn with_where_predicates(
    generics: &syn::Generics,
    predicates: &[syn::WherePredicate],
) -> syn::Generics {
    let mut cloned = generics.clone();
    cloned
        .make_where_clause()
        .predicates
        .extend(predicates.iter().cloned());
    cloned
}

pub fn with_where_predicates_from_fields<F>(
    item: &ast::Input,
    generics: &syn::Generics,
    from_field: F,
) -> syn::Generics
where
    F: Fn(&attr::Field) -> Option<&[syn::WherePredicate]>,
{
    let mut cloned = generics.clone();
    {
        let fields = item.body.all_fields();
        let field_where_predicates = fields
            .iter()
            .flat_map(|field| from_field(&field.attrs))
            .flat_map(|predicates| predicates.to_vec());

        cloned
            .make_where_clause()
            .predicates
            .extend(field_where_predicates);
    }
    cloned
}

/// Puts the given bound on any generic type parameters that are used in fields
/// for which filter returns true.
///
/// For example, the following structure needs the bound `A: Debug, B: Debug`.
///
/// ```ignore
/// struct S<'b, A, B: 'b, C> {
///     a: A,
///     b: Option<&'b B>
///     #[derivative(Debug="ignore")]
///     c: C,
/// }
/// ```
pub fn with_bound<F>(
    item: &ast::Input,
    generics: &syn::Generics,
    filter: F,
    bound: &syn::Path,
) -> syn::Generics
where
    F: Fn(&attr::Field) -> bool,
{
    #[derive(Debug)]
    struct FindTyParams {
        /// Set of all generic type parameters on the current struct (A, B, C in
        /// the example). Initialized up front.
        all_ty_params: HashSet<syn::Ident>,
        /// Set of generic type parameters used in fields for which filter
        /// returns true (A and B in the example). Filled in as the visitor sees
        /// them.
        relevant_ty_params: HashSet<syn::Ident>,
    }
    impl<'ast> visit::Visit<'ast> for FindTyParams {
        fn visit_path(&mut self, path: &'ast syn::Path) {
            if is_phantom_data(path) {
                // Hardcoded exception, because `PhantomData<T>` implements
                // most traits whether or not `T` implements it.
                return;
            }
            if path.leading_colon.is_none() && path.segments.len() == 1 {
                let id = &path.segments[0].ident;
                if self.all_ty_params.contains(id) {
                    self.relevant_ty_params.insert(id.clone());
                }
            }
            visit::visit_path(self, path);
        }
    }

    let all_ty_params: HashSet<_> = generics
        .type_params()
        .map(|ty_param| ty_param.ident.clone())
        .collect();

    let relevant_tys = item
        .body
        .all_fields()
        .into_iter()
        .filter(|field| {
            if let syn::Type::Path(syn::TypePath { ref path, .. }) = *field.ty {
                !is_phantom_data(path)
            } else {
                true
            }
        })
        .filter(|field| filter(&field.attrs))
        .map(|field| &field.ty);

    let mut visitor = FindTyParams {
        all_ty_params,
        relevant_ty_params: HashSet::new(),
    };
    for ty in relevant_tys {
        visit::visit_type(&mut visitor, ty);
    }

    let mut cloned = generics.clone();
    {
        let relevant_where_predicates = generics
            .type_params()
            .map(|ty_param| &ty_param.ident)
            .filter(|id| visitor.relevant_ty_params.contains(id))
            .map(|id| -> syn::WherePredicate { parse_quote!( #id : #bound ) });

        cloned
            .make_where_clause()
            .predicates
            .extend(relevant_where_predicates);
    }
    cloned
}

#[allow(clippy::match_like_matches_macro)] // needs rustc 1.42
fn is_phantom_data(path: &syn::Path) -> bool {
    match path.segments.last() {
        Some(path) if path.ident == "PhantomData" => true,
        _ => false,
    }
}
