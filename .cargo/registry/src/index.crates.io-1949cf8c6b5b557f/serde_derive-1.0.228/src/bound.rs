use crate::internals::ast::{Container, Data};
use crate::internals::{attr, ungroup};
use proc_macro2::Span;
use std::collections::HashSet;
use syn::punctuated::{Pair, Punctuated};
use syn::Token;

// Remove the default from every type parameter because in the generated impls
// they look like associated types: "error: associated type bindings are not
// allowed here".
pub fn without_defaults(generics: &syn::Generics) -> syn::Generics {
    syn::Generics {
        params: generics
            .params
            .iter()
            .map(|param| match param {
                syn::GenericParam::Type(param) => syn::GenericParam::Type(syn::TypeParam {
                    eq_token: None,
                    default: None,
                    ..param.clone()
                }),
                _ => param.clone(),
            })
            .collect(),
        ..generics.clone()
    }
}

pub fn with_where_predicates(
    generics: &syn::Generics,
    predicates: &[syn::WherePredicate],
) -> syn::Generics {
    let mut generics = generics.clone();
    generics
        .make_where_clause()
        .predicates
        .extend(predicates.iter().cloned());
    generics
}

pub fn with_where_predicates_from_fields(
    cont: &Container,
    generics: &syn::Generics,
    from_field: fn(&attr::Field) -> Option<&[syn::WherePredicate]>,
) -> syn::Generics {
    let predicates = cont
        .data
        .all_fields()
        .filter_map(|field| from_field(&field.attrs))
        .flat_map(<[syn::WherePredicate]>::to_vec);

    let mut generics = generics.clone();
    generics.make_where_clause().predicates.extend(predicates);
    generics
}

pub fn with_where_predicates_from_variants(
    cont: &Container,
    generics: &syn::Generics,
    from_variant: fn(&attr::Variant) -> Option<&[syn::WherePredicate]>,
) -> syn::Generics {
    let variants = match &cont.data {
        Data::Enum(variants) => variants,
        Data::Struct(_, _) => {
            return generics.clone();
        }
    };

    let predicates = variants
        .iter()
        .filter_map(|variant| from_variant(&variant.attrs))
        .flat_map(<[syn::WherePredicate]>::to_vec);

    let mut generics = generics.clone();
    generics.make_where_clause().predicates.extend(predicates);
    generics
}

// Puts the given bound on any generic type parameters that are used in fields
// for which filter returns true.
//
// For example, the following struct needs the bound `A: Serialize, B:
// Serialize`.
//
//     struct S<'b, A, B: 'b, C> {
//         a: A,
//         b: Option<&'b B>
//         #[serde(skip_serializing)]
//         c: C,
//     }
pub fn with_bound(
    cont: &Container,
    generics: &syn::Generics,
    filter: fn(&attr::Field, Option<&attr::Variant>) -> bool,
    bound: &syn::Path,
) -> syn::Generics {
    struct FindTyParams<'ast> {
        // Set of all generic type parameters on the current struct (A, B, C in
        // the example). Initialized up front.
        all_type_params: HashSet<syn::Ident>,

        // Set of generic type parameters used in fields for which filter
        // returns true (A and B in the example). Filled in as the visitor sees
        // them.
        relevant_type_params: HashSet<syn::Ident>,

        // Fields whose type is an associated type of one of the generic type
        // parameters.
        associated_type_usage: Vec<&'ast syn::TypePath>,
    }

    impl<'ast> FindTyParams<'ast> {
        fn visit_field(&mut self, field: &'ast syn::Field) {
            if let syn::Type::Path(ty) = ungroup(&field.ty) {
                if let Some(Pair::Punctuated(t, _)) = ty.path.segments.pairs().next() {
                    if self.all_type_params.contains(&t.ident) {
                        self.associated_type_usage.push(ty);
                    }
                }
            }
            self.visit_type(&field.ty);
        }

        fn visit_path(&mut self, path: &'ast syn::Path) {
            if let Some(seg) = path.segments.last() {
                if seg.ident == "PhantomData" {
                    // Hardcoded exception, because PhantomData<T> implements
                    // Serialize and Deserialize whether or not T implements it.
                    return;
                }
            }
            if path.leading_colon.is_none() && path.segments.len() == 1 {
                let id = &path.segments[0].ident;
                if self.all_type_params.contains(id) {
                    self.relevant_type_params.insert(id.clone());
                }
            }
            for segment in &path.segments {
                self.visit_path_segment(segment);
            }
        }

        // Everything below is simply traversing the syntax tree.

        fn visit_type(&mut self, ty: &'ast syn::Type) {
            match ty {
                #![cfg_attr(all(test, exhaustive), deny(non_exhaustive_omitted_patterns))]
                syn::Type::Array(ty) => self.visit_type(&ty.elem),
                syn::Type::BareFn(ty) => {
                    for arg in &ty.inputs {
                        self.visit_type(&arg.ty);
                    }
                    self.visit_return_type(&ty.output);
                }
                syn::Type::Group(ty) => self.visit_type(&ty.elem),
                syn::Type::ImplTrait(ty) => {
                    for bound in &ty.bounds {
                        self.visit_type_param_bound(bound);
                    }
                }
                syn::Type::Macro(ty) => self.visit_macro(&ty.mac),
                syn::Type::Paren(ty) => self.visit_type(&ty.elem),
                syn::Type::Path(ty) => {
                    if let Some(qself) = &ty.qself {
                        self.visit_type(&qself.ty);
                    }
                    self.visit_path(&ty.path);
                }
                syn::Type::Ptr(ty) => self.visit_type(&ty.elem),
                syn::Type::Reference(ty) => self.visit_type(&ty.elem),
                syn::Type::Slice(ty) => self.visit_type(&ty.elem),
                syn::Type::TraitObject(ty) => {
                    for bound in &ty.bounds {
                        self.visit_type_param_bound(bound);
                    }
                }
                syn::Type::Tuple(ty) => {
                    for elem in &ty.elems {
                        self.visit_type(elem);
                    }
                }

                syn::Type::Infer(_) | syn::Type::Never(_) | syn::Type::Verbatim(_) => {}

                _ => {}
            }
        }

        fn visit_path_segment(&mut self, segment: &'ast syn::PathSegment) {
            self.visit_path_arguments(&segment.arguments);
        }

        fn visit_path_arguments(&mut self, arguments: &'ast syn::PathArguments) {
            match arguments {
                syn::PathArguments::None => {}
                syn::PathArguments::AngleBracketed(arguments) => {
                    for arg in &arguments.args {
                        match arg {
                            #![cfg_attr(all(test, exhaustive), deny(non_exhaustive_omitted_patterns))]
                            syn::GenericArgument::Type(arg) => self.visit_type(arg),
                            syn::GenericArgument::AssocType(arg) => self.visit_type(&arg.ty),
                            syn::GenericArgument::Lifetime(_)
                            | syn::GenericArgument::Const(_)
                            | syn::GenericArgument::AssocConst(_)
                            | syn::GenericArgument::Constraint(_) => {}
                            _ => {}
                        }
                    }
                }
                syn::PathArguments::Parenthesized(arguments) => {
                    for argument in &arguments.inputs {
                        self.visit_type(argument);
                    }
                    self.visit_return_type(&arguments.output);
                }
            }
        }

        fn visit_return_type(&mut self, return_type: &'ast syn::ReturnType) {
            match return_type {
                syn::ReturnType::Default => {}
                syn::ReturnType::Type(_, output) => self.visit_type(output),
            }
        }

        fn visit_type_param_bound(&mut self, bound: &'ast syn::TypeParamBound) {
            match bound {
                #![cfg_attr(all(test, exhaustive), deny(non_exhaustive_omitted_patterns))]
                syn::TypeParamBound::Trait(bound) => self.visit_path(&bound.path),
                syn::TypeParamBound::Lifetime(_)
                | syn::TypeParamBound::PreciseCapture(_)
                | syn::TypeParamBound::Verbatim(_) => {}
                _ => {}
            }
        }

        // Type parameter should not be considered used by a macro path.
        //
        //     struct TypeMacro<T> {
        //         mac: T!(),
        //         marker: PhantomData<T>,
        //     }
        fn visit_macro(&mut self, _mac: &'ast syn::Macro) {}
    }

    let all_type_params = generics
        .type_params()
        .map(|param| param.ident.clone())
        .collect();

    let mut visitor = FindTyParams {
        all_type_params,
        relevant_type_params: HashSet::new(),
        associated_type_usage: Vec::new(),
    };
    match &cont.data {
        Data::Enum(variants) => {
            for variant in variants {
                let relevant_fields = variant
                    .fields
                    .iter()
                    .filter(|field| filter(&field.attrs, Some(&variant.attrs)));
                for field in relevant_fields {
                    visitor.visit_field(field.original);
                }
            }
        }
        Data::Struct(_, fields) => {
            for field in fields.iter().filter(|field| filter(&field.attrs, None)) {
                visitor.visit_field(field.original);
            }
        }
    }

    let relevant_type_params = visitor.relevant_type_params;
    let associated_type_usage = visitor.associated_type_usage;
    let new_predicates = generics
        .type_params()
        .map(|param| param.ident.clone())
        .filter(|id| relevant_type_params.contains(id))
        .map(|id| syn::TypePath {
            qself: None,
            path: id.into(),
        })
        .chain(associated_type_usage.into_iter().cloned())
        .map(|bounded_ty| {
            syn::WherePredicate::Type(syn::PredicateType {
                lifetimes: None,
                // the type parameter that is being bounded e.g. T
                bounded_ty: syn::Type::Path(bounded_ty),
                colon_token: <Token![:]>::default(),
                // the bound e.g. Serialize
                bounds: vec![syn::TypeParamBound::Trait(syn::TraitBound {
                    paren_token: None,
                    modifier: syn::TraitBoundModifier::None,
                    lifetimes: None,
                    path: bound.clone(),
                })]
                .into_iter()
                .collect(),
            })
        });

    let mut generics = generics.clone();
    generics
        .make_where_clause()
        .predicates
        .extend(new_predicates);
    generics
}

pub fn with_self_bound(
    cont: &Container,
    generics: &syn::Generics,
    bound: &syn::Path,
) -> syn::Generics {
    let mut generics = generics.clone();
    generics
        .make_where_clause()
        .predicates
        .push(syn::WherePredicate::Type(syn::PredicateType {
            lifetimes: None,
            // the type that is being bounded e.g. MyStruct<'a, T>
            bounded_ty: type_of_item(cont),
            colon_token: <Token![:]>::default(),
            // the bound e.g. Default
            bounds: vec![syn::TypeParamBound::Trait(syn::TraitBound {
                paren_token: None,
                modifier: syn::TraitBoundModifier::None,
                lifetimes: None,
                path: bound.clone(),
            })]
            .into_iter()
            .collect(),
        }));
    generics
}

pub fn with_lifetime_bound(generics: &syn::Generics, lifetime: &str) -> syn::Generics {
    let bound = syn::Lifetime::new(lifetime, Span::call_site());
    let def = syn::LifetimeParam {
        attrs: Vec::new(),
        lifetime: bound.clone(),
        colon_token: None,
        bounds: Punctuated::new(),
    };

    let params = Some(syn::GenericParam::Lifetime(def))
        .into_iter()
        .chain(generics.params.iter().cloned().map(|mut param| {
            match &mut param {
                syn::GenericParam::Lifetime(param) => {
                    param.bounds.push(bound.clone());
                }
                syn::GenericParam::Type(param) => {
                    param
                        .bounds
                        .push(syn::TypeParamBound::Lifetime(bound.clone()));
                }
                syn::GenericParam::Const(_) => {}
            }
            param
        }))
        .collect();

    syn::Generics {
        params,
        ..generics.clone()
    }
}

fn type_of_item(cont: &Container) -> syn::Type {
    syn::Type::Path(syn::TypePath {
        qself: None,
        path: syn::Path {
            leading_colon: None,
            segments: vec![syn::PathSegment {
                ident: cont.ident.clone(),
                arguments: syn::PathArguments::AngleBracketed(
                    syn::AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: <Token![<]>::default(),
                        args: cont
                            .generics
                            .params
                            .iter()
                            .map(|param| match param {
                                syn::GenericParam::Type(param) => {
                                    syn::GenericArgument::Type(syn::Type::Path(syn::TypePath {
                                        qself: None,
                                        path: param.ident.clone().into(),
                                    }))
                                }
                                syn::GenericParam::Lifetime(param) => {
                                    syn::GenericArgument::Lifetime(param.lifetime.clone())
                                }
                                syn::GenericParam::Const(_) => {
                                    panic!("Serde does not support const generics yet");
                                }
                            })
                            .collect(),
                        gt_token: <Token![>]>::default(),
                    },
                ),
            }]
            .into_iter()
            .collect(),
        },
    })
}
