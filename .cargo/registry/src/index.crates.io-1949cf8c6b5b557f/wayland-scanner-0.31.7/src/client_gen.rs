use proc_macro2::{Ident, Span, TokenStream};

use quote::{format_ident, quote};

use crate::{
    protocol::{Interface, Protocol, Type},
    util::{description_to_doc_attr, dotted_to_relname, is_keyword, snake_to_camel, to_doc_attr},
    Side,
};

pub fn generate_client_objects(protocol: &Protocol) -> TokenStream {
    protocol.interfaces.iter().map(generate_objects_for).collect()
}

fn generate_objects_for(interface: &Interface) -> TokenStream {
    let mod_name = Ident::new(&interface.name, Span::call_site());
    let mod_doc = interface.description.as_ref().map(description_to_doc_attr);
    let iface_name = Ident::new(&snake_to_camel(&interface.name), Span::call_site());
    let iface_const_name = format_ident!("{}_INTERFACE", interface.name.to_ascii_uppercase());

    let enums = crate::common::generate_enums_for(interface);
    let sinces = crate::common::gen_msg_constants(&interface.requests, &interface.events);

    let requests = crate::common::gen_message_enum(
        &format_ident!("Request"),
        Side::Client,
        false,
        &interface.requests,
    );
    let events = crate::common::gen_message_enum(
        &format_ident!("Event"),
        Side::Client,
        true,
        &interface.events,
    );

    let parse_body = crate::common::gen_parse_body(interface, Side::Client);
    let write_body = crate::common::gen_write_body(interface, Side::Client);
    let methods = gen_methods(interface);

    let event_ref = if interface.events.is_empty() {
        "This interface has no events."
    } else {
        "See also the [Event] enum for this interface."
    };
    let docs = match &interface.description {
        Some((short, long)) => format!("{short}\n\n{long}\n\n{event_ref}"),
        None => format!("{}\n\n{}", interface.name, event_ref),
    };
    let doc_attr = to_doc_attr(&docs);

    quote! {
        #mod_doc
        pub mod #mod_name {
            use std::sync::Arc;
            use std::os::unix::io::OwnedFd;

            use super::wayland_client::{
                backend::{
                    Backend, WeakBackend, smallvec, ObjectData, ObjectId, InvalidId,
                    protocol::{WEnum, Argument, Message, Interface, same_interface}
                },
                QueueProxyData, Proxy, Connection, Dispatch, QueueHandle, DispatchError, Weak,
            };

            #enums
            #sinces
            #requests
            #events

            #doc_attr
            #[derive(Debug, Clone)]
            pub struct #iface_name {
                id: ObjectId,
                version: u32,
                data: Option<Arc<dyn ObjectData>>,
                backend: WeakBackend,
            }

            impl std::cmp::PartialEq for #iface_name {
                fn eq(&self, other: &#iface_name) -> bool {
                    self.id == other.id
                }
            }

            impl std::cmp::Eq for #iface_name {}

            impl PartialEq<Weak<#iface_name>> for #iface_name {
                fn eq(&self, other: &Weak<#iface_name>) -> bool {
                    self.id == other.id()
                }
            }

            impl std::borrow::Borrow<ObjectId> for #iface_name {
                fn borrow(&self) -> &ObjectId {
                    &self.id
                }
            }

            impl std::hash::Hash for #iface_name {
                fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                    self.id.hash(state)
                }
            }

            impl super::wayland_client::Proxy for #iface_name {
                type Request<'request> = Request<'request>;
                type Event = Event;

                #[inline]
                fn interface() -> &'static Interface{
                    &super::#iface_const_name
                }

                #[inline]
                fn id(&self) -> ObjectId {
                    self.id.clone()
                }

                #[inline]
                fn version(&self) -> u32 {
                    self.version
                }

                #[inline]
                fn data<U: Send + Sync + 'static>(&self) -> Option<&U> {
                    self.data.as_ref().and_then(|arc| arc.data_as_any().downcast_ref::<U>())
                }

                fn object_data(&self) -> Option<&Arc<dyn ObjectData>> {
                    self.data.as_ref()
                }

                fn backend(&self) -> &WeakBackend {
                    &self.backend
                }

                fn send_request(&self, req: Self::Request<'_>) -> Result<(), InvalidId> {
                    let conn = Connection::from_backend(self.backend.upgrade().ok_or(InvalidId)?);
                    let id = conn.send_request(self, req, None)?;
                    debug_assert!(id.is_null());
                    Ok(())
                }

                fn send_constructor<I: Proxy>(&self, req: Self::Request<'_>, data: Arc<dyn ObjectData>) -> Result<I, InvalidId> {
                    let conn = Connection::from_backend(self.backend.upgrade().ok_or(InvalidId)?);
                    let id = conn.send_request(self, req, Some(data))?;
                    Proxy::from_id(&conn, id)
                }

                #[inline]
                fn from_id(conn: &Connection, id: ObjectId) -> Result<Self, InvalidId> {
                    if !same_interface(id.interface(), Self::interface()) && !id.is_null() {
                        return Err(InvalidId);
                    }
                    let version = conn.object_info(id.clone()).map(|info| info.version).unwrap_or(0);
                    let data = conn.get_object_data(id.clone()).ok();
                    let backend = conn.backend().downgrade();
                    Ok(#iface_name { id, data, version, backend })
                }

                #[inline]
                fn inert(backend: WeakBackend) -> Self {
                    #iface_name { id: ObjectId::null(), data: None, version: 0, backend }
                }

                fn parse_event(conn: &Connection, msg: Message<ObjectId, OwnedFd>) -> Result<(Self, Self::Event), DispatchError> {
                    #parse_body
                }

                fn write_request<'a>(&self, conn: &Connection, msg: Self::Request<'a>) -> Result<(Message<ObjectId, std::os::unix::io::BorrowedFd<'a>>, Option<(&'static Interface, u32)>), InvalidId> {
                    #write_body
                }
            }

            impl #iface_name {
                #methods
            }
        }
    }
}

fn gen_methods(interface: &Interface) -> TokenStream {
    interface.requests.iter().map(|request| {
        let created_interface = request.args.iter().find(|arg| arg.typ == Type::NewId).map(|arg| &arg.interface);

        let method_name = format_ident!("{}{}", if is_keyword(&request.name) { "_" } else { "" }, request.name);
        let enum_variant = Ident::new(&snake_to_camel(&request.name), Span::call_site());

        let fn_args = request.args.iter().flat_map(|arg| {
            if arg.typ == Type::NewId {
                if arg.interface.is_none() {
                    // the new_id argument of a bind-like method
                    // it shoudl expand as a (interface, type) tuple, but the type is already handled by
                    // the prototype type parameter, so just put a version here
                    return Some(quote! { version: u32 });
                } else {
                    // this is a regular new_id, skip it
                    return None;
                }
            }

            let arg_name = format_ident!("{}{}", if is_keyword(&arg.name) { "_" } else { "" }, arg.name);

            let arg_type =  if let Some(ref enu) = arg.enum_ {
                let enum_type = dotted_to_relname(enu);
                quote! { #enum_type }
            } else {
                match arg.typ {
                    Type::Uint => quote! { u32 },
                    Type::Int => quote! { i32 },
                    Type::Fixed => quote! { f64 },
                    Type::String => if arg.allow_null { quote!{ Option<String> } } else { quote!{ String } },
                    Type::Array => if arg.allow_null { quote!{ Option<Vec<u8>> } } else { quote!{ Vec<u8> } },
                    Type::Fd => quote! { ::std::os::unix::io::BorrowedFd<'_> },
                    Type::Object => {
                        let iface = arg.interface.as_ref().unwrap();
                        let iface_mod = Ident::new(iface, Span::call_site());
                        let iface_type =
                            Ident::new(&snake_to_camel(iface), Span::call_site());
                        if arg.allow_null { quote! { Option<&super::#iface_mod::#iface_type> } } else { quote! { &super::#iface_mod::#iface_type } }
                    },
                    Type::NewId => unreachable!(),
                    Type::Destructor => panic!("An argument cannot have type \"destructor\"."),
                }
            };

            Some(quote! {
                #arg_name: #arg_type
            })
        });

        let enum_args = request.args.iter().flat_map(|arg| {
            let arg_name = format_ident!("{}{}", if is_keyword(&arg.name) { "_" } else { "" }, arg.name);
            if arg.enum_.is_some() {
                Some(quote! { #arg_name: WEnum::Value(#arg_name) })
            } else if arg.typ == Type::NewId {
                if arg.interface.is_none() {
                    Some(quote! { #arg_name: (I::interface(), version) })
                } else {
                    None
                }
            } else if arg.typ == Type::Object {
                if arg.allow_null {
                    Some(quote! { #arg_name: #arg_name.cloned() })
                } else {
                    Some(quote! { #arg_name: #arg_name.clone() })
                }
            } else {
                Some(quote! { #arg_name })
            }
        });

        let doc_attr = request
            .description
            .as_ref()
            .map(description_to_doc_attr);

        match created_interface {
            Some(Some(ref created_interface)) => {
                // a regular creating request
                let created_iface_mod = Ident::new(created_interface, Span::call_site());
                let created_iface_type = Ident::new(&snake_to_camel(created_interface), Span::call_site());
                quote! {
                    #doc_attr
                    #[allow(clippy::too_many_arguments)]
                    pub fn #method_name<U: Send + Sync + 'static, D: Dispatch<super::#created_iface_mod::#created_iface_type, U> + 'static>(&self, #(#fn_args,)* qh: &QueueHandle<D>, udata: U) -> super::#created_iface_mod::#created_iface_type {
                        self.send_constructor(
                            Request::#enum_variant {
                                #(#enum_args),*
                            },
                            qh.make_data::<super::#created_iface_mod::#created_iface_type, U>(udata),
                        ).unwrap_or_else(|_| Proxy::inert(self.backend.clone()))
                    }
                }
            },
            Some(None) => {
                // a bind-like request
                quote! {
                    #doc_attr
                    #[allow(clippy::too_many_arguments)]
                    pub fn #method_name<I: Proxy + 'static, U: Send + Sync + 'static, D: Dispatch<I, U> + 'static>(&self, #(#fn_args,)* qh: &QueueHandle<D>, udata: U) -> I {
                        self.send_constructor(
                            Request::#enum_variant {
                                #(#enum_args),*
                            },
                            qh.make_data::<I, U>(udata),
                        ).unwrap_or_else(|_| Proxy::inert(self.backend.clone()))
                    }
                }
            },
            None => {
                // a non-creating request
                quote! {
                    #doc_attr
                    #[allow(clippy::too_many_arguments)]
                    pub fn #method_name(&self, #(#fn_args),*) {
                        let backend = match self.backend.upgrade() {
                            Some(b) => b,
                            None => return,
                        };
                        let conn = Connection::from_backend(backend);
                        let _ = conn.send_request(
                            self,
                            Request::#enum_variant {
                                #(#enum_args),*
                            },
                            None
                        );
                    }
                }
            }
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn client_gen() {
        let protocol_file =
            std::fs::File::open("./tests/scanner_assets/test-protocol.xml").unwrap();
        let protocol_parsed = crate::parse::parse(protocol_file);
        let generated: String = super::generate_client_objects(&protocol_parsed).to_string();
        let generated = crate::format_rust_code(&generated);

        let reference =
            std::fs::read_to_string("./tests/scanner_assets/test-client-code.rs").unwrap();
        let reference = crate::format_rust_code(&reference);

        if reference != generated {
            let diff = similar::TextDiff::from_lines(&reference, &generated);
            print!("{}", diff.unified_diff().context_radius(10).header("reference", "generated"));
            panic!("Generated does not match reference!")
        }
    }
}
