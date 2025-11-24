use proc_macro2::TokenStream;
use quote::{quote, ToTokens};

use crate::util::PathList;

use super::ForwardAttrs;

/// Infrastructure for generating an attribute extractor.
pub trait ExtractAttribute {
    /// A set of mutable declarations for all members of the implementing type.
    fn local_declarations(&self) -> TokenStream;

    /// Gets the list of attribute names that should be parsed by the extractor.
    fn attr_names(&self) -> &PathList;

    fn forward_attrs(&self) -> &ForwardAttrs<'_>;

    /// Gets the name used by the generated impl to return to the `syn` item passed as input.
    fn param_name(&self) -> TokenStream;

    /// Get the tokens to access a borrowed list of attributes where extraction will take place.
    ///
    /// By default, this will be `&#input.attrs` where `#input` is `self.param_name()`.
    fn attrs_accessor(&self) -> TokenStream {
        let input = self.param_name();
        quote!(&#input.attrs)
    }

    /// Gets the core from-meta-item loop that should be used on matching attributes.
    fn core_loop(&self) -> TokenStream;

    /// Generates the main extraction loop.
    fn extractor(&self) -> TokenStream {
        let mut declarations = self.local_declarations();
        self.forward_attrs()
            .as_declaration()
            .to_tokens(&mut declarations);

        let will_parse_any = !self.attr_names().is_empty();

        // Forwarding requires both that there be some items we would forward,
        // and a place that will keep the forwarded items.
        let will_fwd_any = self.forward_attrs().will_forward_any();

        if !(will_parse_any || will_fwd_any) {
            return quote! {
                #declarations
            };
        }

        let attrs_accessor = self.attrs_accessor();

        // The block for parsing attributes whose names have been claimed by the target
        // struct. If no attributes were claimed, this is a pass-through.
        let parse_handled = if will_parse_any {
            let attr_names = self.attr_names().to_strings();
            let core_loop = self.core_loop();
            quote!(
                #(#attr_names)|* => {
                    match ::darling::util::parse_attribute_to_meta_list(__attr) {
                        ::darling::export::Ok(__data) => {
                            match ::darling::export::NestedMeta::parse_meta_list(__data.tokens) {
                                ::darling::export::Ok(ref __items) => {
                                    if __items.is_empty() {
                                        continue;
                                    }

                                    #core_loop
                                }
                                ::darling::export::Err(__err) => {
                                    __errors.push(__err.into());
                                }
                            }
                        }
                        // darling was asked to handle this attribute name, but the actual attribute
                        // isn't one that darling can work with. This either indicates a typing error
                        // or some misunderstanding of the meta attribute syntax; in either case, the
                        // caller should get a useful error.
                        ::darling::export::Err(__err) => {
                            __errors.push(__err);
                        }
                    }
                }
            )
        } else {
            quote!()
        };

        let fwd_population = self.forward_attrs().as_value_populator();

        // Specifies the behavior for unhandled attributes. They will either be silently ignored or
        // forwarded to the inner struct for later analysis.
        let forward_unhandled = self.forward_attrs().as_match_arms();

        quote!(
            #declarations
            use ::darling::ToTokens;

            for __attr in #attrs_accessor {
                // Filter attributes based on name
                match ::darling::export::ToString::to_string(&__attr.path().clone().into_token_stream()).as_str() {
                    #parse_handled
                    #forward_unhandled
                }
            }

            #fwd_population
        )
    }
}
