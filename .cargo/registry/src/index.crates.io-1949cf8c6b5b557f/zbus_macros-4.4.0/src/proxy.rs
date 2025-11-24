use crate::utils::{pat_ident, typed_arg, zbus_path, PropertyEmitsChangedSignal};
use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned, ToTokens};
use syn::{
    fold::Fold, parse_quote, parse_str, punctuated::Punctuated, spanned::Spanned, Error, FnArg,
    Ident, ItemTrait, Meta, Path, ReturnType, Token, TraitItemFn,
};
use zvariant_utils::{case, def_attrs, macros::AttrParse, old_new};

pub mod old {
    use super::def_attrs;
    def_attrs! {
        crate dbus_proxy;

        pub TraitAttributes("trait") {
            interface str,
            name str,
            assume_defaults bool,
            default_path str,
            default_service str,
            async_name str,
            blocking_name str,
            gen_async bool,
            gen_blocking bool
        };

        pub MethodAttributes("method") {
            name str,
            property {
                pub PropertyAttributes("property") {
                    emits_changed_signal str
                }
            },
            signal none,
            object str,
            async_object str,
            blocking_object str,
            no_reply none,
            no_autostart none,
            allow_interactive_auth none
        };
    }
}

def_attrs! {
    crate zbus;

    // Keep this in sync with interface's proxy method attributes.
    pub TraitAttributes("trait") {
        interface str,
        name str,
        assume_defaults bool,
        default_path str,
        default_service str,
        async_name str,
        blocking_name str,
        gen_async bool,
        gen_blocking bool
    };

    // Keep this in sync with interface's proxy method attributes.
    pub MethodAttributes("method") {
        name str,
        property {
            pub PropertyAttributes("property") {
                emits_changed_signal str
            }
        },
        signal none,
        object str,
        async_object str,
        blocking_object str,
        no_reply none,
        no_autostart none,
        allow_interactive_auth none
    };
}

old_new!(TraitAttrs, old::TraitAttributes, TraitAttributes);
old_new!(MethodAttrs, old::MethodAttributes, MethodAttributes);

struct AsyncOpts {
    blocking: bool,
    usage: TokenStream,
    wait: TokenStream,
}

impl AsyncOpts {
    fn new(blocking: bool) -> Self {
        let (usage, wait) = if blocking {
            (quote! {}, quote! {})
        } else {
            (quote! { async }, quote! { .await })
        };
        Self {
            blocking,
            usage,
            wait,
        }
    }
}

pub fn expand<I: AttrParse + Into<TraitAttrs>, M: AttrParse + Into<MethodAttrs>>(
    args: Punctuated<Meta, Token![,]>,
    input: ItemTrait,
) -> Result<TokenStream, Error> {
    let (
        interface,
        name,
        assume_defaults,
        default_path,
        default_service,
        async_name,
        blocking_name,
        gen_async,
        gen_blocking,
    ) = match I::parse_nested_metas(args)?.into() {
        TraitAttrs::Old(old) => (
            old.interface,
            old.name,
            old.assume_defaults,
            old.default_path,
            old.default_service,
            old.async_name,
            old.blocking_name,
            old.gen_async,
            old.gen_blocking,
        ),
        TraitAttrs::New(new) => (
            new.interface,
            new.name,
            new.assume_defaults,
            new.default_path,
            new.default_service,
            new.async_name,
            new.blocking_name,
            new.gen_async,
            new.gen_blocking,
        ),
    };

    let iface_name = match (interface, name) {
        (Some(name), None) | (None, Some(name)) => Ok(Some(name)),
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err(syn::Error::new(
            input.span(),
            "both `interface` and `name` attributes shouldn't be specified at the same time",
        )),
    }?;
    let gen_async = gen_async.unwrap_or(true);
    let gen_blocking = gen_blocking.unwrap_or(true);

    // Some sanity checks
    assert!(
        gen_blocking || gen_async,
        "Can't disable both asynchronous and blocking proxy. 😸",
    );
    assert!(
        gen_blocking || blocking_name.is_none(),
        "Can't set blocking proxy's name if you disabled it. 😸",
    );
    assert!(
        gen_async || async_name.is_none(),
        "Can't set asynchronous proxy's name if you disabled it. 😸",
    );

    let blocking_proxy = if gen_blocking {
        let proxy_name = blocking_name.unwrap_or_else(|| {
            if gen_async {
                format!("{}ProxyBlocking", input.ident)
            } else {
                // When only generating blocking proxy, there is no need for a suffix.
                format!("{}Proxy", input.ident)
            }
        });
        create_proxy::<M>(
            &input,
            iface_name.as_deref(),
            assume_defaults,
            default_path.as_deref(),
            default_service.as_deref(),
            &proxy_name,
            true,
            // Signal args structs are shared between the two proxies so always generate it for
            // async proxy only unless async proxy generation is disabled.
            !gen_async,
        )?
    } else {
        quote! {}
    };
    let async_proxy = if gen_async {
        let proxy_name = async_name.unwrap_or_else(|| format!("{}Proxy", input.ident));
        create_proxy::<M>(
            &input,
            iface_name.as_deref(),
            assume_defaults,
            default_path.as_deref(),
            default_service.as_deref(),
            &proxy_name,
            false,
            true,
        )?
    } else {
        quote! {}
    };

    Ok(quote! {
        #blocking_proxy

        #async_proxy
    })
}

#[allow(clippy::too_many_arguments)]
pub fn create_proxy<M: AttrParse + Into<MethodAttrs>>(
    input: &ItemTrait,
    iface_name: Option<&str>,
    assume_defaults: Option<bool>,
    default_path: Option<&str>,
    default_service: Option<&str>,
    proxy_name: &str,
    blocking: bool,
    gen_sig_args: bool,
) -> Result<TokenStream, Error> {
    let zbus = zbus_path();

    let other_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("zbus") && !a.path().is_ident("dbus_proxy"))
        .collect();
    let proxy_name = Ident::new(proxy_name, Span::call_site());
    let ident = input.ident.to_string();
    let iface_name = iface_name
        .map(ToString::to_string)
        .unwrap_or(format!("org.freedesktop.{ident}"));
    let assume_defaults = assume_defaults.unwrap_or(false);
    let (default_path, default_service) = if assume_defaults {
        let path = default_path
            .map(ToString::to_string)
            .or_else(|| Some(format!("/org/freedesktop/{ident}")));
        let svc = default_service
            .map(ToString::to_string)
            .or_else(|| Some(iface_name.clone()));
        (path, svc)
    } else {
        let path = default_path.map(ToString::to_string);
        let svc = default_service.map(ToString::to_string);
        (path, svc)
    };
    let mut methods = TokenStream::new();
    let mut stream_types = TokenStream::new();
    let mut has_properties = false;
    let mut uncached_properties: Vec<String> = vec![];

    let async_opts = AsyncOpts::new(blocking);

    for i in input.items.iter() {
        if let syn::TraitItem::Fn(m) = i {
            let (mut name, signal, property) = match <M>::parse(&m.attrs)?.into() {
                MethodAttrs::Old(old) => (
                    old.name,
                    old.signal,
                    old.property.map(|property| property.emits_changed_signal),
                ),
                MethodAttrs::New(new) => (
                    new.name,
                    new.signal,
                    new.property.map(|property| property.emits_changed_signal),
                ),
            };

            let method_name = m.sig.ident.to_string();

            let is_signal = signal;
            let is_property = property.is_some();
            let has_inputs = m.sig.inputs.len() > 1;

            let member_name = name.take().unwrap_or_else(|| {
                case::pascal_or_camel_case(
                    if is_property && has_inputs {
                        assert!(method_name.starts_with("set_"));
                        &method_name[4..]
                    } else {
                        &method_name
                    },
                    true,
                )
            });

            let m = if let Some(prop_attrs) = &property {
                has_properties = true;

                let emits_changed_signal = if let Some(s) = &prop_attrs {
                    PropertyEmitsChangedSignal::parse(s, m.span())?
                } else {
                    PropertyEmitsChangedSignal::True
                };

                if let PropertyEmitsChangedSignal::False = emits_changed_signal {
                    uncached_properties.push(member_name.clone());
                }

                gen_proxy_property(
                    &member_name,
                    &method_name,
                    m,
                    &async_opts,
                    emits_changed_signal,
                )
            } else if is_signal {
                let (method, types) = gen_proxy_signal(
                    &proxy_name,
                    &iface_name,
                    &member_name,
                    &method_name,
                    m,
                    &async_opts,
                    gen_sig_args,
                );
                stream_types.extend(types);

                method
            } else {
                gen_proxy_method_call::<M>(
                    &member_name,
                    &method_name,
                    m,
                    <M>::parse(&m.attrs)?,
                    &async_opts,
                )?
            };
            methods.extend(m);
        }
    }

    let AsyncOpts { usage, wait, .. } = async_opts;
    let (proxy_struct, connection, builder, proxy_trait) = if blocking {
        let connection = quote! { #zbus::blocking::Connection };
        let proxy = quote! { #zbus::blocking::Proxy };
        let builder = quote! { #zbus::blocking::proxy::Builder };
        let proxy_trait = quote! { #zbus::blocking::proxy::ProxyImpl };

        (proxy, connection, builder, proxy_trait)
    } else {
        let connection = quote! { #zbus::Connection };
        let proxy = quote! { #zbus::Proxy };
        let builder = quote! { #zbus::proxy::Builder };
        let proxy_trait = quote! { #zbus::proxy::ProxyImpl };

        (proxy, connection, builder, proxy_trait)
    };

    let proxy_method_new = match (&default_path, &default_service) {
        (None, None) => {
            quote! {
                /// Creates a new proxy with the given service destination and path.
                pub #usage fn new<D, P>(conn: &#connection, destination: D, path: P) -> #zbus::Result<#proxy_name<'p>>
                where
                    D: ::std::convert::TryInto<#zbus::names::BusName<'static>>,
                    D::Error: ::std::convert::Into<#zbus::Error>,
                    P: ::std::convert::TryInto<#zbus::zvariant::ObjectPath<'static>>,
                    P::Error: ::std::convert::Into<#zbus::Error>,
                {
                    let obj_path = path.try_into().map_err(::std::convert::Into::into)?;
                    let obj_destination = destination.try_into().map_err(::std::convert::Into::into)?;
                    Self::builder(conn)
                        .path(obj_path)?
                        .destination(obj_destination)?
                        .build()#wait
                }
            }
        }
        (Some(_), None) => {
            quote! {
                /// Creates a new proxy with the given destination, and the default path.
                pub #usage fn new<D>(conn: &#connection, destination: D) -> #zbus::Result<#proxy_name<'p>>
                where
                    D: ::std::convert::TryInto<#zbus::names::BusName<'static>>,
                    D::Error: ::std::convert::Into<#zbus::Error>,
                {
                    let obj_dest = destination.try_into().map_err(::std::convert::Into::into)?;
                    Self::builder(conn)
                        .destination(obj_dest)?
                        .build()#wait
                }
            }
        }
        (None, Some(_)) => {
            quote! {
                /// Creates a new proxy with the given path, and the default destination.
                pub #usage fn new<P>(conn: &#connection, path: P) -> #zbus::Result<#proxy_name<'p>>
                where
                    P: ::std::convert::TryInto<#zbus::zvariant::ObjectPath<'static>>,
                    P::Error: ::std::convert::Into<#zbus::Error>,
                {
                    let obj_path = path.try_into().map_err(::std::convert::Into::into)?;
                    Self::builder(conn)
                        .path(obj_path)?
                        .build()#wait
                }
            }
        }
        (Some(_), Some(_)) => {
            quote! {
                /// Creates a new proxy with the default service and path.
                pub #usage fn new(conn: &#connection) -> #zbus::Result<#proxy_name<'p>> {
                    Self::builder(conn).build()#wait
                }
            }
        }
    };
    let default_path = match default_path {
        Some(p) => quote! { Some(#p) },
        None => quote! { None },
    };
    let default_service = match default_service {
        Some(d) => quote! { Some(#d) },
        None => quote! { None },
    };

    Ok(quote! {
        impl<'a> #zbus::proxy::ProxyDefault for #proxy_name<'a> {
            const INTERFACE: Option<&'static str> = Some(#iface_name);
            const DESTINATION: Option<&'static str> = #default_service;
            const PATH: Option<&'static str> = #default_path;
        }

        #(#other_attrs)*
        #[derive(Clone, Debug)]
        pub struct #proxy_name<'p>(#proxy_struct<'p>);

        impl<'p> #proxy_name<'p> {
            #proxy_method_new

            /// Returns a customizable builder for this proxy.
            pub fn builder(conn: &#connection) -> #builder<'p, Self> {
                let mut builder = #builder::new(conn) ;
                if #has_properties {
                    let uncached = vec![#(#uncached_properties),*];
                    builder.cache_properties(#zbus::proxy::CacheProperties::default())
                           .uncached_properties(&uncached)
                } else {
                    builder.cache_properties(#zbus::proxy::CacheProperties::No)
                }
            }

            /// Consumes `self`, returning the underlying `zbus::Proxy`.
            pub fn into_inner(self) -> #proxy_struct<'p> {
                self.0
            }

            /// The reference to the underlying `zbus::Proxy`.
            pub fn inner(&self) -> &#proxy_struct<'p> {
                &self.0
            }

            /// The mutable reference to the underlying `zbus::Proxy`.
            pub fn inner_mut(&mut self) -> &mut #proxy_struct<'p> {
                &mut self.0
            }

            #methods
        }

        impl<'p> #proxy_trait<'p> for #proxy_name<'p> {
            fn builder(conn: &#connection) -> #builder<'p, Self> {
                Self::builder(conn)
            }

            fn into_inner(self) -> #proxy_struct<'p> {
                self.into_inner()
            }

            fn inner(&self) -> &#proxy_struct<'p> {
                self.inner()
            }
        }

        impl<'p> ::std::convert::From<#zbus::Proxy<'p>> for #proxy_name<'p> {
            fn from(proxy: #zbus::Proxy<'p>) -> Self {
                #proxy_name(::std::convert::Into::into(proxy))
            }
        }

        impl<'p> ::std::convert::AsRef<#proxy_struct<'p>> for #proxy_name<'p> {
            fn as_ref(&self) -> &#proxy_struct<'p> {
                self.inner()
            }
        }

        impl<'p> ::std::convert::AsMut<#proxy_struct<'p>> for #proxy_name<'p> {
            fn as_mut(&mut self) -> &mut #proxy_struct<'p> {
                self.inner_mut()
            }
        }

        impl<'p> #zbus::zvariant::Type for #proxy_name<'p> {
            fn signature() -> #zbus::zvariant::Signature<'static> {
                #zbus::zvariant::OwnedObjectPath::signature()
            }
        }

        impl<'p> #zbus::export::serde::ser::Serialize for #proxy_name<'p> {
            fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
            where
                S: #zbus::export::serde::ser::Serializer,
            {
                ::std::string::String::serialize(
                    &::std::string::ToString::to_string(self.inner().path()),
                    serializer,
                )
            }
        }

        #stream_types
    })
}

fn gen_proxy_method_call<M: AttrParse + Into<MethodAttrs>>(
    method_name: &str,
    snake_case_name: &str,
    m: &TraitItemFn,
    method_attrs: M,
    async_opts: &AsyncOpts,
) -> Result<TokenStream, Error> {
    let (object, blocking_object, async_object, no_reply, no_autostart, allow_interactive_auth) =
        match method_attrs.into() {
            MethodAttrs::Old(old) => (
                old.object,
                old.blocking_object,
                old.async_object,
                old.no_reply,
                old.no_autostart,
                old.allow_interactive_auth,
            ),
            MethodAttrs::New(new) => (
                new.object,
                new.blocking_object,
                new.async_object,
                new.no_reply,
                new.no_autostart,
                new.allow_interactive_auth,
            ),
        };
    let AsyncOpts {
        usage,
        wait,
        blocking,
    } = async_opts;
    let zbus = zbus_path();
    let other_attrs: Vec<_> = m
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("zbus") && !a.path().is_ident("dbus_proxy"))
        .collect();
    let args: Vec<_> = m
        .sig
        .inputs
        .iter()
        .filter_map(typed_arg)
        .filter_map(pat_ident)
        .collect();

    let proxy_object = object.as_ref().map(|o| {
        if *blocking {
            // FIXME: for some reason Rust doesn't let us move `blocking_proxy_object` so we've to
            // clone.
            blocking_object
                .as_ref()
                .cloned()
                .unwrap_or_else(|| format!("{o}ProxyBlocking"))
        } else {
            async_object
                .as_ref()
                .cloned()
                .unwrap_or_else(|| format!("{o}Proxy"))
        }
    });

    let method_flags = match (no_reply, no_autostart, allow_interactive_auth) {
        (true, false, false) => Some(quote!(::std::convert::Into::into(
            zbus::proxy::MethodFlags::NoReplyExpected
        ))),
        (false, true, false) => Some(quote!(::std::convert::Into::into(
            zbus::proxy::MethodFlags::NoAutoStart
        ))),
        (false, false, true) => Some(quote!(::std::convert::Into::into(
            zbus::proxy::MethodFlags::AllowInteractiveAuth
        ))),

        (true, true, false) => Some(quote!(
            zbus::proxy::MethodFlags::NoReplyExpected | zbus::proxy::MethodFlags::NoAutoStart
        )),
        (true, false, true) => Some(quote!(
            zbus::proxy::MethodFlags::NoReplyExpected
                | zbus::proxy::MethodFlags::AllowInteractiveAuth
        )),
        (false, true, true) => Some(quote!(
            zbus::proxy::MethodFlags::NoAutoStart | zbus::proxy::MethodFlags::AllowInteractiveAuth
        )),

        (true, true, true) => Some(quote!(
            zbus::proxy::MethodFlags::NoReplyExpected
                | zbus::proxy::MethodFlags::NoAutoStart
                | zbus::proxy::MethodFlags::AllowInteractiveAuth
        )),
        _ => None,
    };

    let method = Ident::new(snake_case_name, Span::call_site());
    let inputs = &m.sig.inputs;
    let mut generics = m.sig.generics.clone();
    let where_clause = generics.where_clause.get_or_insert(parse_quote!(where));
    for param in generics
        .params
        .iter()
        .filter(|a| matches!(a, syn::GenericParam::Type(_)))
    {
        let is_input_type = inputs.iter().any(|arg| {
            // FIXME: We want to only require `Serialize` from input types and `DeserializeOwned`
            // from output types but since we don't have type introspection, we employ this
            // workaround of matching on the string representation of the the types to figure out
            // which generic types are input types.
            if let FnArg::Typed(pat) = arg {
                let pat = pat.ty.to_token_stream().to_string();

                if let Some(ty_name) = pat.strip_prefix('&') {
                    let ty_name = ty_name.trim_start();

                    ty_name == param.to_token_stream().to_string()
                } else {
                    false
                }
            } else {
                false
            }
        });
        let serde_bound: TokenStream = if is_input_type {
            parse_quote!(#zbus::export::serde::ser::Serialize)
        } else {
            parse_quote!(#zbus::export::serde::de::DeserializeOwned)
        };
        where_clause.predicates.push(parse_quote!(
            #param: #serde_bound + #zbus::zvariant::Type
        ));
    }
    let (_, ty_generics, where_clause) = generics.split_for_impl();

    if let Some(proxy_path) = proxy_object {
        let proxy_path = parse_str::<Path>(&proxy_path)?;
        let signature = quote! {
            fn #method #ty_generics(#inputs) -> #zbus::Result<#proxy_path<'p>>
            #where_clause
        };

        Ok(quote! {
            #(#other_attrs)*
            pub #usage #signature {
                let object_path: #zbus::zvariant::OwnedObjectPath =
                    self.0.call(
                        #method_name,
                        &#zbus::zvariant::DynamicTuple((#(#args,)*)),
                    )
                    #wait?;
                #proxy_path::builder(&self.0.connection())
                    .path(object_path)?
                    .build()
                    #wait
            }
        })
    } else {
        let body = if args.len() == 1 {
            // Wrap single arg in a tuple so if it's a struct/tuple itself, zbus will only remove
            // the '()' from the signature that we add and not the actual intended ones.
            let arg = &args[0];
            quote! {
                &#zbus::zvariant::DynamicTuple((#arg,))
            }
        } else {
            quote! {
                &#zbus::zvariant::DynamicTuple((#(#args),*))
            }
        };

        let output = &m.sig.output;
        let signature = quote! {
            fn #method #ty_generics(#inputs) #output
            #where_clause
        };

        if let Some(method_flags) = method_flags {
            if no_reply {
                Ok(quote! {
                    #(#other_attrs)*
                    pub #usage #signature {
                        self.0.call_with_flags::<_, _, ()>(#method_name, #method_flags, #body)#wait?;
                        ::std::result::Result::Ok(())
                    }
                })
            } else {
                Ok(quote! {
                    #(#other_attrs)*
                    pub #usage #signature {
                        let reply = self.0.call_with_flags(#method_name, #method_flags, #body)#wait?;

                        // SAFETY: This unwrap() cannot fail due to the guarantees in
                        // call_with_flags, which can only return Ok(None) if the
                        // NoReplyExpected is set. By not passing NoReplyExpected,
                        // we are guaranteed to get either an Err variant (handled
                        // in the previous statement) or Ok(Some(T)) which is safe to
                        // unwrap
                        ::std::result::Result::Ok(reply.unwrap())
                    }
                })
            }
        } else {
            Ok(quote! {
                #(#other_attrs)*
                pub #usage #signature {
                    let reply = self.0.call(#method_name, #body)#wait?;
                    ::std::result::Result::Ok(reply)
                }
            })
        }
    }
}

fn gen_proxy_property(
    property_name: &str,
    method_name: &str,
    m: &TraitItemFn,
    async_opts: &AsyncOpts,
    emits_changed_signal: PropertyEmitsChangedSignal,
) -> TokenStream {
    let AsyncOpts {
        usage,
        wait,
        blocking,
    } = async_opts;
    let zbus = zbus_path();
    let other_attrs: Vec<_> = m
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("zbus") && !a.path().is_ident("dbus_proxy"))
        .collect();
    let signature = &m.sig;
    if signature.inputs.len() > 1 {
        let value = pat_ident(typed_arg(signature.inputs.last().unwrap()).unwrap()).unwrap();
        quote! {
            #(#other_attrs)*
            #[allow(clippy::needless_question_mark)]
            pub #usage #signature {
                ::std::result::Result::Ok(self.0.set_property(#property_name, #value)#wait?)
            }
        }
    } else {
        // This should fail to compile only if the return type is wrong,
        // so use that as the span.
        let body_span = if let ReturnType::Type(_, ty) = &signature.output {
            ty.span()
        } else {
            signature.span()
        };
        let body = quote_spanned! {body_span =>
            ::std::result::Result::Ok(self.0.get_property(#property_name)#wait?)
        };
        let ret_type = if let ReturnType::Type(_, ty) = &signature.output {
            Some(ty)
        } else {
            None
        };

        let (proxy_name, prop_stream) = if *blocking {
            (
                "zbus::blocking::Proxy",
                quote! { #zbus::blocking::proxy::PropertyIterator },
            )
        } else {
            ("zbus::Proxy", quote! { #zbus::proxy::PropertyStream })
        };

        let receive_method = match emits_changed_signal {
            PropertyEmitsChangedSignal::True | PropertyEmitsChangedSignal::Invalidates => {
                let (_, ty_generics, where_clause) = m.sig.generics.split_for_impl();
                let receive = format_ident!("receive_{}_changed", method_name);
                let gen_doc = format!(
                    "Create a stream for the `{property_name}` property changes. \
                This is a convenient wrapper around [`{proxy_name}::receive_property_changed`]."
                );
                quote! {
                    #[doc = #gen_doc]
                    pub #usage fn #receive #ty_generics(
                        &self
                    ) -> #prop_stream<'p, <#ret_type as #zbus::ResultAdapter>::Ok>
                    #where_clause
                    {
                        self.0.receive_property_changed(#property_name)#wait
                    }
                }
            }
            PropertyEmitsChangedSignal::False | PropertyEmitsChangedSignal::Const => {
                quote! {}
            }
        };

        let cached_getter_method = match emits_changed_signal {
            PropertyEmitsChangedSignal::True
            | PropertyEmitsChangedSignal::Invalidates
            | PropertyEmitsChangedSignal::Const => {
                let cached_getter = format_ident!("cached_{}", method_name);
                let cached_doc = format!(
                    " Get the cached value of the `{property_name}` property, or `None` if the property is not cached.",
                );
                quote! {
                    #[doc = #cached_doc]
                    pub fn #cached_getter(&self) -> ::std::result::Result<
                        ::std::option::Option<<#ret_type as #zbus::ResultAdapter>::Ok>,
                        <#ret_type as #zbus::ResultAdapter>::Err>
                    {
                        self.0.cached_property(#property_name).map_err(::std::convert::Into::into)
                    }
                }
            }
            PropertyEmitsChangedSignal::False => quote! {},
        };

        quote! {
            #(#other_attrs)*
            #[allow(clippy::needless_question_mark)]
            pub #usage #signature {
                #body
            }

            #cached_getter_method

            #receive_method
        }
    }
}

struct SetLifetimeS;

impl Fold for SetLifetimeS {
    fn fold_type_reference(&mut self, node: syn::TypeReference) -> syn::TypeReference {
        let mut t = syn::fold::fold_type_reference(self, node);
        t.lifetime = Some(syn::Lifetime::new("'s", Span::call_site()));
        t
    }

    fn fold_lifetime(&mut self, _node: syn::Lifetime) -> syn::Lifetime {
        syn::Lifetime::new("'s", Span::call_site())
    }
}

fn gen_proxy_signal(
    proxy_name: &Ident,
    iface_name: &str,
    signal_name: &str,
    snake_case_name: &str,
    method: &TraitItemFn,
    async_opts: &AsyncOpts,
    gen_sig_args: bool,
) -> (TokenStream, TokenStream) {
    let AsyncOpts {
        usage,
        wait,
        blocking,
    } = async_opts;
    let zbus = zbus_path();
    let other_attrs: Vec<_> = method
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("zbus") && !a.path().is_ident("dbus_proxy"))
        .collect();
    let input_types: Vec<_> = method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(p) => Some(&*p.ty),
            _ => None,
        })
        .collect();
    let input_types_s: Vec<_> = SetLifetimeS
        .fold_signature(method.sig.clone())
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(p) => Some(p.ty.clone()),
            _ => None,
        })
        .collect();
    let args: Vec<Ident> = method
        .sig
        .inputs
        .iter()
        .filter_map(typed_arg)
        .filter_map(|arg| pat_ident(arg).cloned())
        .collect();
    let args_nth: Vec<Literal> = args
        .iter()
        .enumerate()
        .map(|(i, _)| Literal::usize_unsuffixed(i))
        .collect();

    let mut generics = method.sig.generics.clone();
    let where_clause = generics.where_clause.get_or_insert(parse_quote!(where));
    for param in generics
        .params
        .iter()
        .filter(|a| matches!(a, syn::GenericParam::Type(_)))
    {
        where_clause
                .predicates
                .push(parse_quote!(#param: #zbus::export::serde::de::Deserialize<'s> + #zbus::zvariant::Type + ::std::fmt::Debug));
    }
    generics.params.push(parse_quote!('s));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (
        proxy_path,
        receive_signal_link,
        receive_signal_with_args_link,
        trait_name,
        trait_link,
        signal_type,
    ) = if *blocking {
        (
            "zbus::blocking::Proxy",
            "https://docs.rs/zbus/latest/zbus/blocking/proxy/struct.Proxy.html#method.receive_signal",
            "https://docs.rs/zbus/latest/zbus/blocking/proxy/struct.Proxy.html#method.receive_signal_with_args",
            "Iterator",
            "https://doc.rust-lang.org/std/iter/trait.Iterator.html",
            quote! { blocking::proxy::SignalIterator },
        )
    } else {
        (
            "zbus::Proxy",
            "https://docs.rs/zbus/latest/zbus/proxy/struct.Proxy.html#method.receive_signal",
            "https://docs.rs/zbus/latest/zbus/proxy/struct.Proxy.html#method.receive_signal_with_args",
            "Stream",
            "https://docs.rs/futures/0.3.15/futures/stream/trait.Stream.html",
            quote! { proxy::SignalStream },
        )
    };
    let receiver_name = format_ident!("receive_{snake_case_name}");
    let receiver_with_args_name = format_ident!("receive_{snake_case_name}_with_args");
    let stream_name = format_ident!("{signal_name}{trait_name}");
    let signal_args = format_ident!("{signal_name}Args");
    let signal_name_ident = format_ident!("{signal_name}");

    let receive_gen_doc = format!(
        "Create a stream that receives `{signal_name}` signals.\n\
            \n\
            This a convenient wrapper around [`{proxy_path}::receive_signal`]({receive_signal_link}).",
    );
    let receive_with_args_gen_doc = format!(
        "Create a stream that receives `{signal_name}` signals.\n\
            \n\
            This a convenient wrapper around [`{proxy_path}::receive_signal_with_args`]({receive_signal_with_args_link}).",
    );
    let receive_signal_with_args = if args.is_empty() {
        quote!()
    } else {
        quote! {
            #[doc = #receive_with_args_gen_doc]
            #(#other_attrs)*
            pub #usage fn #receiver_with_args_name(&self, args: &[(u8, &str)]) -> #zbus::Result<#stream_name<'static>>
            {
                self.0.receive_signal_with_args(#signal_name, args)#wait.map(#stream_name)
            }
        }
    };
    let receive_signal = quote! {
        #[doc = #receive_gen_doc]
        #(#other_attrs)*
        pub #usage fn #receiver_name(&self) -> #zbus::Result<#stream_name<'static>>
        {
            self.0.receive_signal(#signal_name)#wait.map(#stream_name)
        }

        #receive_signal_with_args
    };

    let stream_gen_doc = format!(
        "A [`{trait_name}`] implementation that yields [`{signal_name}`] signals.\n\
            \n\
            Use [`{proxy_name}::{receiver_name}`] to create an instance of this type.\n\
            \n\
            [`{trait_name}`]: {trait_link}",
    );
    let signal_args_gen_doc = format!("`{signal_name}` signal arguments.");
    let args_struct_gen_doc = format!("A `{signal_name}` signal.");
    let args_struct_decl = if gen_sig_args {
        quote! {
            #[doc = #args_struct_gen_doc]
            #[derive(Debug, Clone)]
            pub struct #signal_name_ident(#zbus::message::Body);

            impl #signal_name_ident {
                #[doc = "Try to construct a "]
                #[doc = #signal_name]
                #[doc = " from a [`zbus::Message`]."]
                pub fn from_message<M>(msg: M) -> ::std::option::Option<Self>
                where
                    M: ::std::convert::Into<#zbus::message::Message>,
                {
                    let msg = msg.into();
                    let hdr = msg.header();
                    let message_type = msg.message_type();
                    let interface = hdr.interface();
                    let member = hdr.member();
                    let interface = interface.as_ref().map(|i| i.as_str());
                    let member = member.as_ref().map(|m| m.as_str());

                    match (message_type, interface, member) {
                        (#zbus::message::Type::Signal, Some(#iface_name), Some(#signal_name)) => {
                            Some(Self(msg.body()))
                        }
                        _ => None,
                    }
                }

                #[doc = "The reference to the underlying [`zbus::Message`]."]
                pub fn message(&self) -> &#zbus::message::Message {
                    self.0.message()
                }
            }

            impl ::std::convert::From<#signal_name_ident> for #zbus::message::Message {
                fn from(signal: #signal_name_ident) -> Self {
                    signal.0.message().clone()
                }
            }
        }
    } else {
        quote!()
    };
    let args_impl = if args.is_empty() || !gen_sig_args {
        quote!()
    } else {
        let arg_fields_init = if args.len() == 1 {
            quote! { #(#args)*: args }
        } else {
            quote! { #(#args: args.#args_nth),* }
        };

        quote! {
            impl #signal_name_ident {
                /// Retrieve the signal arguments.
                pub fn args #ty_generics(&'s self) -> #zbus::Result<#signal_args #ty_generics>
                #where_clause
                {
                    ::std::convert::TryFrom::try_from(&self.0)
                }
            }

            #[doc = #signal_args_gen_doc]
            pub struct #signal_args #ty_generics {
                phantom: std::marker::PhantomData<&'s ()>,
                #(
                    pub #args: #input_types_s
                 ),*
            }

            impl #impl_generics #signal_args #ty_generics
                #where_clause
            {
                #(
                    pub fn #args(&self) -> &#input_types_s {
                        &self.#args
                    }
                 )*
            }

            impl #impl_generics std::fmt::Debug for #signal_args #ty_generics
                #where_clause
            {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(#signal_name)
                    #(
                     .field(stringify!(#args), &self.#args)
                    )*
                     .finish()
                }
            }

            impl #impl_generics ::std::convert::TryFrom<&'s #zbus::message::Body> for #signal_args #ty_generics
                #where_clause
            {
                type Error = #zbus::Error;

                fn try_from(msg_body: &'s #zbus::message::Body) -> #zbus::Result<Self> {
                    msg_body.deserialize::<(#(#input_types),*)>()
                        .map_err(::std::convert::Into::into)
                        .map(|args| {
                            #signal_args {
                                phantom: ::std::marker::PhantomData,
                                #arg_fields_init
                            }
                        })
                }
            }
        }
    };
    let stream_impl = if *blocking {
        quote! {
            impl ::std::iter::Iterator for #stream_name<'_> {
                type Item = #signal_name_ident;

                fn next(&mut self) -> ::std::option::Option<Self::Item> {
                    ::std::iter::Iterator::next(&mut self.0)
                        .map(|msg| #signal_name_ident(msg.body()))
                }
            }
        }
    } else {
        quote! {
            impl #zbus::export::futures_core::stream::Stream for #stream_name<'_> {
                type Item = #signal_name_ident;

                fn poll_next(
                    self: ::std::pin::Pin<&mut Self>,
                    cx: &mut ::std::task::Context<'_>,
                    ) -> ::std::task::Poll<::std::option::Option<Self::Item>> {
                    #zbus::export::futures_core::stream::Stream::poll_next(
                        ::std::pin::Pin::new(&mut self.get_mut().0),
                        cx,
                    )
                    .map(|msg| msg.map(|msg| #signal_name_ident(msg.body())))
                }
            }

            impl #zbus::export::ordered_stream::OrderedStream for #stream_name<'_> {
                type Data = #signal_name_ident;
                type Ordering = #zbus::message::Sequence;

                fn poll_next_before(
                    self: ::std::pin::Pin<&mut Self>,
                    cx: &mut ::std::task::Context<'_>,
                    before: ::std::option::Option<&Self::Ordering>
                    ) -> ::std::task::Poll<#zbus::export::ordered_stream::PollResult<Self::Ordering, Self::Data>> {
                    #zbus::export::ordered_stream::OrderedStream::poll_next_before(
                        ::std::pin::Pin::new(&mut self.get_mut().0),
                        cx,
                        before,
                    )
                    .map(|msg| msg.map_data(|msg| #signal_name_ident(msg.body())))
                }
            }

            impl #zbus::export::futures_core::stream::FusedStream for #stream_name<'_> {
                fn is_terminated(&self) -> bool {
                    self.0.is_terminated()
                }
            }

            #[#zbus::export::async_trait::async_trait]
            impl #zbus::AsyncDrop for #stream_name<'_> {
                async fn async_drop(self) {
                    self.0.async_drop().await
                }
            }
        }
    };
    let stream_types = quote! {
        #[doc = #stream_gen_doc]
        #[derive(Debug)]
        pub struct #stream_name<'a>(#zbus::#signal_type<'a>);

        #zbus::export::static_assertions::assert_impl_all!(
            #stream_name<'_>: ::std::marker::Send, ::std::marker::Unpin
        );

        impl<'a> #stream_name<'a> {
            /// Consumes `self`, returning the underlying `zbus::#signal_type`.
            pub fn into_inner(self) -> #zbus::#signal_type<'a> {
                self.0
            }

            /// The reference to the underlying `zbus::#signal_type`.
            pub fn inner(&self) -> & #zbus::#signal_type<'a> {
                &self.0
            }
        }

        #stream_impl

        #args_struct_decl

        #args_impl
    };

    (receive_signal, stream_types)
}
