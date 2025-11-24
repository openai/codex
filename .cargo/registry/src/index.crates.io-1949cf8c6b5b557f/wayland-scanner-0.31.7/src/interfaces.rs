use proc_macro2::TokenStream;

use crate::protocol::{Interface, Message, Protocol, Type};

use quote::{format_ident, quote};

pub fn generate(protocol: &Protocol, with_c_interfaces: bool) -> TokenStream {
    let interfaces =
        protocol.interfaces.iter().map(|iface| generate_interface(iface, with_c_interfaces));
    if with_c_interfaces {
        let prefix = super::c_interfaces::generate_interfaces_prefix(protocol);
        quote! {
            #prefix
            #(#interfaces)*
        }
    } else {
        interfaces.collect()
    }
}

pub(crate) fn generate_interface(interface: &Interface, with_c: bool) -> TokenStream {
    let const_name = format_ident!("{}_INTERFACE", interface.name.to_ascii_uppercase());
    let iface_name = &interface.name;
    let iface_version = interface.version;
    let requests = build_messagedesc_list(&interface.requests);
    let events = build_messagedesc_list(&interface.events);

    let c_name = format_ident!("{}_interface", interface.name);

    if with_c {
        let c_iface = super::c_interfaces::generate_interface(interface);
        quote! {
            pub static #const_name: wayland_backend::protocol::Interface = wayland_backend::protocol::Interface {
                name: #iface_name,
                version: #iface_version,
                requests: #requests,
                events: #events,
                c_ptr: Some(unsafe { & #c_name }),
            };

            #c_iface
        }
    } else {
        quote! {
            pub static #const_name: wayland_backend::protocol::Interface = wayland_backend::protocol::Interface {
                name: #iface_name,
                version: #iface_version,
                requests: #requests,
                events: #events,
                c_ptr: None,
            };
        }
    }
}

fn build_messagedesc_list(list: &[Message]) -> TokenStream {
    let desc_list = list.iter().map(|message| {
        let name = &message.name;
        let since = message.since;
        let is_destructor = message.typ == Some(Type::Destructor);
        let signature = message.args.iter().map(|arg| {
            if arg.typ == Type::NewId && arg.interface.is_none() {
                // this is a special generic message, it expands to multiple arguments
                quote! {
                    wayland_backend::protocol::ArgumentType::Str(wayland_backend::protocol::AllowNull::No),
                    wayland_backend::protocol::ArgumentType::Uint,
                    wayland_backend::protocol::ArgumentType::NewId
                }
            } else {
                let typ = arg.typ.common_type();
                if arg.typ.nullable() {
                    if arg.allow_null {
                        quote! { wayland_backend::protocol::ArgumentType::#typ(wayland_backend::protocol::AllowNull::Yes) }
                    } else {
                        quote! { wayland_backend::protocol::ArgumentType::#typ(wayland_backend::protocol::AllowNull::No) }
                    }
                } else {
                    quote! { wayland_backend::protocol::ArgumentType::#typ }
                }
            }
        });
        let child_interface = match message
            .args
            .iter()
            .find(|arg| arg.typ == Type::NewId)
            .and_then(|arg| arg.interface.as_ref())
        {
            Some(name) => {
                let target_iface = format_ident!("{}_INTERFACE", name.to_ascii_uppercase());
                quote! { Some(&#target_iface) }
            }
            None => quote! { None },
        };
        let arg_interfaces = message.args.iter().filter(|arg| arg.typ == Type::Object).map(|arg| {
            match arg.interface {
                Some(ref name) => {
                    let target_iface = format_ident!("{}_INTERFACE", name.to_ascii_uppercase());
                    quote! { &#target_iface }
                }
                None => {
                    quote! { &wayland_backend::protocol::ANONYMOUS_INTERFACE }
                }
            }
        });
        quote! {
            wayland_backend::protocol::MessageDesc {
                name: #name,
                signature: &[ #(#signature),* ],
                since: #since,
                is_destructor: #is_destructor,
                child_interface: #child_interface,
                arg_interfaces: &[ #(#arg_interfaces),* ],
            }
        }
    });

    quote!(
        &[ #(#desc_list),* ]
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn interface_gen() {
        let protocol_file =
            std::fs::File::open("./tests/scanner_assets/test-protocol.xml").unwrap();
        let protocol_parsed = crate::parse::parse(protocol_file);
        let generated: String = super::generate(&protocol_parsed, true).to_string();
        let generated = crate::format_rust_code(&generated);

        let reference =
            std::fs::read_to_string("./tests/scanner_assets/test-interfaces.rs").unwrap();
        let reference = crate::format_rust_code(&reference);

        if reference != generated {
            let diff = similar::TextDiff::from_lines(&reference, &generated);
            print!("{}", diff.unified_diff().context_radius(10).header("reference", "generated"));
            panic!("Generated does not match reference!")
        }
    }
}
