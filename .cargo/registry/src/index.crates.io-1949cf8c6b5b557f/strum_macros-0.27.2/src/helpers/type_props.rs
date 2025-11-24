use proc_macro2::TokenStream;
use quote::quote;
use std::default::Default;
use syn::{parse_quote, DeriveInput, Ident, LitStr, Path, Visibility};

use super::case_style::CaseStyle;
use super::metadata::{DeriveInputExt, EnumDiscriminantsMeta, EnumMeta};
use super::occurrence_error;

pub trait HasTypeProperties {
    fn get_type_properties(&self) -> syn::Result<StrumTypeProperties>;
}

#[derive(Clone, Default)]
pub struct StrumTypeProperties {
    pub parse_err_ty: Option<Path>,
    pub parse_err_fn: Option<Path>,
    pub case_style: Option<CaseStyle>,
    pub ascii_case_insensitive: bool,
    pub crate_module_path: Option<Path>,
    pub discriminant_derives: Vec<Path>,
    pub discriminant_name: Option<Ident>,
    pub discriminant_others: Vec<TokenStream>,
    pub discriminant_vis: Option<Visibility>,
    pub use_phf: bool,
    pub prefix: Option<LitStr>,
    pub suffix: Option<LitStr>,
    pub enum_repr: Option<TokenStream>,
    pub const_into_str: bool,
    pub discriminant_docs: Vec<LitStr>,
}

impl HasTypeProperties for DeriveInput {
    fn get_type_properties(&self) -> syn::Result<StrumTypeProperties> {
        let mut output = StrumTypeProperties::default();

        let strum_meta = self.get_metadata()?;
        let discriminants_meta = self.get_discriminants_metadata()?;

        let mut parse_err_ty_kw = None;
        let mut parse_err_fn_kw = None;
        let mut serialize_all_kw = None;
        let mut ascii_case_insensitive_kw = None;
        let mut use_phf_kw = None;
        let mut crate_module_path_kw = None;
        let mut prefix_kw = None;
        let mut suffix_kw = None;
        let mut const_into_str = None;

        for meta in strum_meta {
            match meta {
                EnumMeta::SerializeAll { case_style, kw } => {
                    if let Some(fst_kw) = serialize_all_kw {
                        return Err(occurrence_error(fst_kw, kw, "serialize_all"));
                    }

                    serialize_all_kw = Some(kw);
                    output.case_style = Some(case_style);
                }
                EnumMeta::AsciiCaseInsensitive(kw) => {
                    if let Some(fst_kw) = ascii_case_insensitive_kw {
                        return Err(occurrence_error(fst_kw, kw, "ascii_case_insensitive"));
                    }

                    ascii_case_insensitive_kw = Some(kw);
                    output.ascii_case_insensitive = true;
                }
                EnumMeta::UsePhf(kw) => {
                    if let Some(fst_kw) = use_phf_kw {
                        return Err(occurrence_error(fst_kw, kw, "use_phf"));
                    }

                    use_phf_kw = Some(kw);
                    output.use_phf = true;
                }
                EnumMeta::Crate {
                    crate_module_path,
                    kw,
                } => {
                    if let Some(fst_kw) = crate_module_path_kw {
                        return Err(occurrence_error(fst_kw, kw, "Crate"));
                    }

                    crate_module_path_kw = Some(kw);
                    output.crate_module_path = Some(crate_module_path);
                }
                EnumMeta::Prefix { prefix, kw } => {
                    if let Some(fst_kw) = prefix_kw {
                        return Err(occurrence_error(fst_kw, kw, "prefix"));
                    }

                    prefix_kw = Some(kw);
                    output.prefix = Some(prefix);
                }
                EnumMeta::Suffix { suffix, kw } => {
                    if let Some(fst_kw) = suffix_kw {
                        return Err(occurrence_error(fst_kw, kw, "suffix"));
                    }

                    suffix_kw = Some(kw);
                    output.suffix = Some(suffix);
                }
                EnumMeta::ParseErrTy { path, kw } => {
                    if let Some(fst_kw) = parse_err_ty_kw {
                        return Err(occurrence_error(fst_kw, kw, "parse_err_ty"));
                    }

                    parse_err_ty_kw = Some(kw);
                    output.parse_err_ty = Some(path);
                }
                EnumMeta::ParseErrFn { path, kw } => {
                    if let Some(fst_kw) = parse_err_fn_kw {
                        return Err(occurrence_error(fst_kw, kw, "parse_err_fn"));
                    }

                    parse_err_fn_kw = Some(kw);
                    output.parse_err_fn = Some(path);
                }
                EnumMeta::ConstIntoStr(kw) => {
                    if let Some(fst_kw) = const_into_str {
                        return Err(occurrence_error(fst_kw, kw, "const_into_str"));
                    }

                    const_into_str = Some(kw);
                    output.const_into_str = true;
                }
            }
        }

        let mut name_kw = None;
        let mut vis_kw = None;
        for meta in discriminants_meta {
            match meta {
                EnumDiscriminantsMeta::Derive { paths, .. } => {
                    output.discriminant_derives.extend(paths);
                }
                EnumDiscriminantsMeta::Name { name, kw } => {
                    if let Some(fst_kw) = name_kw {
                        return Err(occurrence_error(fst_kw, kw, "name"));
                    }

                    name_kw = Some(kw);
                    output.discriminant_name = Some(name);
                }
                EnumDiscriminantsMeta::Vis { vis, kw } => {
                    if let Some(fst_kw) = vis_kw {
                        return Err(occurrence_error(fst_kw, kw, "vis"));
                    }

                    vis_kw = Some(kw);
                    output.discriminant_vis = Some(vis);
                }
                EnumDiscriminantsMeta::Doc { doc, .. } => {
                    output.discriminant_docs.push(doc);
                }
                EnumDiscriminantsMeta::Other { path, nested } => {
                    output.discriminant_others.push(quote! { #path(#nested) });
                }
            }
        }

        let attrs = &self.attrs;
        for attr in attrs {
            if let Ok(list) = attr.meta.require_list() {
                if let Some(ident) = list.path.get_ident() {
                    if ident == "repr" {
                        output.enum_repr = Some(list.tokens.clone())
                    }
                }
            }
        }

        Ok(output)
    }
}

impl StrumTypeProperties {
    pub fn crate_module_path(&self) -> Path {
        self.crate_module_path
            .as_ref()
            .map_or_else(|| parse_quote!(::strum), |path| parse_quote!(#path))
    }
}
