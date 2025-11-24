#![macro_use]
#![deny(unused)]

use std::collections::{HashMap, HashSet};

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    parse_quote, spanned::Spanned, ConstParam, Expr, GenericParam, Generics, Item, LifetimeParam,
    Path, Result, Type, TypeArray, TypeParam, TypeParen, TypePath, TypeReference, TypeSlice,
    TypeTuple, WhereClause, WherePredicate,
};

use crate::{deps::Dependencies, utils::format_generics};

#[macro_use]
mod utils;
mod attr;
mod deps;
mod optional;
mod types;

struct DerivedTS {
    crate_rename: Path,
    ts_name: Expr,
    docs: Vec<Expr>,
    inline: TokenStream,
    inline_flattened: Option<TokenStream>,
    dependencies: Dependencies,
    concrete: HashMap<Ident, Type>,
    bound: Option<Vec<WherePredicate>>,

    export: bool,
    export_to: Option<Expr>,
}

impl DerivedTS {
    fn into_impl(mut self, rust_ty: Ident, generics: Generics) -> TokenStream {
        let export = self
            .export
            .then(|| self.generate_export_test(&rust_ty, &generics));

        let output_path_fn = {
            let ts_name = &self.ts_name;
            // expression of type `String` containing the file path
            let path_string = match &self.export_to {
                Some(dir_or_file) => quote![{
                    let dir_or_file = format!("{}", #dir_or_file);
                    if dir_or_file.ends_with('/') {
                        // export into directory
                        format!("{dir_or_file}{}.ts", #ts_name)
                    } else {
                        // export into provided file
                        format!("{dir_or_file}")
                    }
                }],
                None => quote![format!("{}.ts", #ts_name)],
            };

            quote! {
                fn output_path() -> Option<std::path::PathBuf> {
                    Some(std::path::PathBuf::from(#path_string))
                }
            }
        };

        let crate_rename = self.crate_rename.clone();
        let docs = match &*self.docs {
            [] => None,
            docs => Some(quote! {
                fn docs() -> Option<String> {
                    Some(#crate_rename::format_docs(&[#(#docs),*]))
                }
            }),
        };

        let ident = self.ts_name.clone();
        let impl_start = generate_impl_block_header(
            &crate_rename,
            &rust_ty,
            &generics,
            self.bound.as_deref(),
            &self.dependencies,
        );
        let assoc_type = generate_assoc_type(&rust_ty, &crate_rename, &generics, &self.concrete);
        let name = self.generate_name_fn(&generics);
        let inline = self.generate_inline_fn();
        let decl = self.generate_decl_fn(&rust_ty, &generics);
        let dependencies = &self.dependencies;
        let generics_fn = self.generate_generics_fn(&generics);

        quote! {
            #impl_start {
                #assoc_type
                type OptionInnerType = Self;

                fn ident() -> String {
                    (#ident).to_string()
                }

                #docs
                #name
                #decl
                #inline
                #generics_fn
                #output_path_fn

                fn visit_dependencies(v: &mut impl #crate_rename::TypeVisitor)
                where
                    Self: 'static,
                {
                    #dependencies
                }
            }

            #export
        }
    }

    /// Returns an expression which evaluates to the TypeScript name of the type, including generic
    /// parameters.
    fn name_with_generics(&self, generics: &Generics) -> TokenStream {
        let name = &self.ts_name;
        let crate_rename = &self.crate_rename;
        let mut generics_ts_names = generics
            .type_params()
            .filter(|ty| !self.concrete.contains_key(&ty.ident))
            .map(|ty| &ty.ident)
            .map(|generic| quote!(<#generic as #crate_rename::TS>::name()))
            .peekable();

        if generics_ts_names.peek().is_some() {
            quote! {
                format!("{}<{}>", #name, vec![#(#generics_ts_names),*].join(", "))
            }
        } else {
            quote!((#name).to_string())
        }
    }

    /// Generate a dummy unit struct for every generic type parameter of this type.
    /// # Example:
    /// ```compile_fail
    /// struct Generic<A, B, const C: usize> { /* ... */ }
    /// ```
    /// has two generic type parameters, `A` and `B`. This function will therefore generate
    /// ```compile_fail
    /// struct A;
    /// impl ts_rs::TS for A { /* .. */ }
    ///
    /// struct B;
    /// impl ts_rs::TS for B { /* .. */ }
    /// ```
    fn generate_generic_types(&self, generics: &Generics) -> TokenStream {
        let crate_rename = &self.crate_rename;
        let generics = generics
            .type_params()
            .filter(|ty| !self.concrete.contains_key(&ty.ident))
            .map(|ty| ty.ident.clone());
        let name = quote![<Self as #crate_rename::TS>::name()];
        quote! {
            #(
                #[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
                struct #generics;
                impl std::fmt::Display for #generics {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "{:?}", self)
                    }
                }
                impl #crate_rename::TS for #generics {
                    type WithoutGenerics = #generics;
                    type OptionInnerType = Self;
                    fn name() -> String { stringify!(#generics).to_owned() }
                    fn inline() -> String { panic!("{} cannot be inlined", #name) }
                    fn inline_flattened() -> String { stringify!(#generics).to_owned() }
                    fn decl() -> String { panic!("{} cannot be declared", #name) }
                    fn decl_concrete() -> String { panic!("{} cannot be declared", #name) }
                }
            )*
        }
    }

    fn generate_export_test(&self, rust_ty: &Ident, generics: &Generics) -> TokenStream {
        let test_fn = format_ident!(
            "export_bindings_{}",
            rust_ty.to_string().to_lowercase().replace("r#", "")
        );
        let crate_rename = &self.crate_rename;
        let generic_params = generics
            .type_params()
            .map(|ty| match self.concrete.get(&ty.ident) {
                None => quote! { #crate_rename::Dummy },
                Some(ty) => quote! { #ty },
            });
        let ty = quote!(<#rust_ty<#(#generic_params),*> as #crate_rename::TS>);

        quote! {
            #[cfg(test)]
            #[test]
            fn #test_fn() {
                #ty::export_all().expect("could not export type");
            }
        }
    }

    fn generate_generics_fn(&self, generics: &Generics) -> TokenStream {
        let crate_rename = &self.crate_rename;
        let generics = generics
            .type_params()
            .filter(|ty| !self.concrete.contains_key(&ty.ident))
            .map(|TypeParam { ident, .. }| {
                quote![
                    v.visit::<#ident>();
                    <#ident as #crate_rename::TS>::visit_generics(v);
                ]
            });
        quote! {
            fn visit_generics(v: &mut impl #crate_rename::TypeVisitor)
            where
                Self: 'static,
            {
                #(#generics)*
            }
        }
    }

    fn generate_name_fn(&self, generics: &Generics) -> TokenStream {
        let name = self.name_with_generics(generics);
        quote! {
            fn name() -> String {
                #name
            }
        }
    }

    fn generate_inline_fn(&self) -> TokenStream {
        let inline = &self.inline;
        let crate_rename = &self.crate_rename;

        let inline_flattened = self.inline_flattened.as_ref().map_or_else(
            || {
                quote! {
                    fn inline_flattened() -> String {
                        panic!("{} cannot be flattened", <Self as #crate_rename::TS>::name())
                    }
                }
            },
            |inline_flattened| {
                quote! {
                    fn inline_flattened() -> String {
                        #inline_flattened
                    }
                }
            },
        );
        let inline = quote! {
            fn inline() -> String {
                #inline
            }
        };
        quote! {
            #inline
            #inline_flattened
        }
    }

    /// Generates the `decl()` and `decl_concrete()` methods.
    /// `decl_concrete()` is simple, and simply defers to `inline()`.
    /// For `decl()`, however, we need to change out the generic parameters of the type, replacing
    /// them with the dummy types generated by `generate_generic_types()`.
    fn generate_decl_fn(&mut self, rust_ty: &Ident, generics: &Generics) -> TokenStream {
        let name = &self.ts_name;
        let crate_rename = &self.crate_rename;
        let generic_types = self.generate_generic_types(generics);
        let ts_generics = format_generics(
            &mut self.dependencies,
            crate_rename,
            generics,
            &self.concrete,
        );

        use GenericParam as G;
        // These are the generic parameters we'll be using.
        let generic_idents = generics.params.iter().filter_map(|p| match p {
            G::Lifetime(_) => None,
            G::Type(TypeParam { ident, .. }) => match self.concrete.get(ident) {
                // Since we named our dummy types the same as the generic parameters, we can just keep
                // the identifier of the generic parameter - its name is shadowed by the dummy struct.
                None => Some(quote!(#ident)),
                // If the type parameter is concrete, we use the type the user provided using
                // `#[ts(concrete)]`
                Some(concrete) => Some(quote!(#concrete)),
            },
            // We keep const parameters as they are, since there's no sensible default value we can
            // use instead. This might be something to change in the future.
            G::Const(ConstParam { ident, .. }) => Some(quote!(#ident)),
        });
        quote! {
            fn decl_concrete() -> String {
                format!("type {} = {};", #name, <Self as #crate_rename::TS>::inline())
            }
            fn decl() -> String {
                #generic_types
                let inline = <#rust_ty<#(#generic_idents,)*> as #crate_rename::TS>::inline();
                let generics = #ts_generics;
                format!("type {}{generics} = {inline};", #name)
            }
        }
    }
}

fn generate_assoc_type(
    rust_ty: &Ident,
    crate_rename: &Path,
    generics: &Generics,
    concrete: &HashMap<Ident, Type>,
) -> TokenStream {
    use GenericParam as G;

    let generics_params = generics.params.iter().map(|x| match x {
        G::Type(ty) => match concrete.get(&ty.ident) {
            None => quote! { #crate_rename::Dummy },
            Some(ty) => quote! { #ty },
        },
        G::Const(ConstParam { ident, .. }) => quote! { #ident },
        G::Lifetime(LifetimeParam { lifetime, .. }) => quote! { #lifetime },
    });

    quote! { type WithoutGenerics = #rust_ty<#(#generics_params),*>; }
}

// generate start of the `impl TS for #ty` block, up to (excluding) the open brace
fn generate_impl_block_header(
    crate_rename: &Path,
    ty: &Ident,
    generics: &Generics,
    bounds: Option<&[WherePredicate]>,
    dependencies: &Dependencies,
) -> TokenStream {
    use GenericParam as G;

    let params = generics.params.iter().map(|param| match param {
        G::Type(TypeParam {
            ident,
            colon_token,
            bounds,
            ..
        }) => quote!(#ident #colon_token #bounds),
        G::Lifetime(LifetimeParam {
            lifetime,
            colon_token,
            bounds,
            ..
        }) => quote!(#lifetime #colon_token #bounds),
        G::Const(ConstParam {
            const_token,
            ident,
            colon_token,
            ty,
            ..
        }) => quote!(#const_token #ident #colon_token #ty),
    });
    let type_args = generics.params.iter().map(|param| match param {
        G::Type(TypeParam { ident, .. }) | G::Const(ConstParam { ident, .. }) => quote!(#ident),
        G::Lifetime(LifetimeParam { lifetime, .. }) => quote!(#lifetime),
    });

    let where_bound = match bounds {
        Some(bounds) => quote! { where #(#bounds),* },
        None => {
            let bounds = generate_where_clause(crate_rename, generics, dependencies);
            quote! { #bounds }
        }
    };

    quote!(impl <#(#params),*> #crate_rename::TS for #ty <#(#type_args),*> #where_bound)
}

fn generate_where_clause(
    crate_rename: &Path,
    generics: &Generics,
    dependencies: &Dependencies,
) -> WhereClause {
    let used_types = {
        let is_type_param = |id: &Ident| generics.type_params().any(|p| &p.ident == id);

        let mut used_types = HashSet::new();
        for ty in dependencies.used_types() {
            used_type_params(&mut used_types, ty, is_type_param);
        }
        used_types.into_iter()
    };

    let existing = generics.where_clause.iter().flat_map(|w| &w.predicates);
    parse_quote! {
        where #(#existing,)* #(#used_types: #crate_rename::TS),*
    }
}

// Extracts all type parameters which are used within the given type.
// Associated types of a type parameter are extracted as well.
// Note: This will not extract `I` from `I::Item`, but just `I::Item`!
fn used_type_params<'ty, 'out>(
    out: &'out mut HashSet<&'ty Type>,
    ty: &'ty Type,
    is_type_param: impl Fn(&'ty Ident) -> bool + Copy + 'out,
) {
    use syn::{
        AngleBracketedGenericArguments as GenericArgs, GenericArgument as G, PathArguments as P,
    };

    match ty {
        Type::Array(TypeArray { elem, .. })
        | Type::Paren(TypeParen { elem, .. })
        | Type::Reference(TypeReference { elem, .. })
        | Type::Slice(TypeSlice { elem, .. }) => used_type_params(out, elem, is_type_param),
        Type::Tuple(TypeTuple { elems, .. }) => elems
            .iter()
            .for_each(|elem| used_type_params(out, elem, is_type_param)),
        Type::Path(TypePath { qself: None, path }) => {
            let first = path.segments.first().unwrap();
            if is_type_param(&first.ident) {
                // The type is either a generic parameter (e.g `T`), or an associated type of that
                // generic parameter (e.g `I::Item`). Either way, we return it.
                out.insert(ty);
                return;
            }

            let last = path.segments.last().unwrap();
            if let P::AngleBracketed(GenericArgs { ref args, .. }) = last.arguments {
                for generic in args {
                    if let G::Type(ty) = generic {
                        used_type_params(out, ty, is_type_param);
                    }
                }
            }
        }
        _ => (),
    }
}

/// Derives [TS](./trait.TS.html) for a struct or enum.
/// Please take a look at [TS](./trait.TS.html) for documentation.
#[proc_macro_derive(TS, attributes(ts))]
pub fn typescript(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match entry(input) {
        Err(err) => err.to_compile_error(),
        Ok(result) => result,
    }
    .into()
}

fn entry(input: proc_macro::TokenStream) -> Result<TokenStream> {
    let input = syn::parse::<Item>(input)?;
    let (ts, ident, generics) = match input {
        Item::Struct(s) => (types::struct_def(&s)?, s.ident, s.generics),
        Item::Enum(e) => (types::enum_def(&e)?, e.ident, e.generics),
        _ => syn_err!(input.span(); "unsupported item"),
    };

    Ok(ts.into_impl(ident, generics))
}
