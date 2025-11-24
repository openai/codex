use std::cmp;

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

use crate::protocol::{Interface, Message, Protocol, Type};

pub(crate) fn generate_interfaces_prefix(protocol: &Protocol) -> TokenStream {
    let longest_nulls = protocol.interfaces.iter().fold(0, |max, interface| {
        let request_longest_null = interface.requests.iter().fold(0, |max, request| {
            if request.all_null() {
                cmp::max(request.args.len(), max)
            } else {
                max
            }
        });
        let events_longest_null = interface.events.iter().fold(0, |max, event| {
            if event.all_null() {
                cmp::max(event.args.len(), max)
            } else {
                max
            }
        });
        cmp::max(max, cmp::max(request_longest_null, events_longest_null))
    });

    let types_null_len = Literal::usize_unsuffixed(longest_nulls);

    quote! {
        use std::ptr::null;
        struct SyncWrapper<T>(T);
        unsafe impl<T> Sync for SyncWrapper<T> {}
        static types_null: SyncWrapper<[*const wayland_backend::protocol::wl_interface; #types_null_len]> = SyncWrapper([
            null::<wayland_backend::protocol::wl_interface>(); #types_null_len
        ]);
    }
}

pub(crate) fn generate_interface(interface: &Interface) -> TokenStream {
    let requests = gen_messages(interface, &interface.requests, "requests");
    let events = gen_messages(interface, &interface.events, "events");

    let interface_ident = format_ident!("{}_interface", interface.name);
    let name_value = null_terminated_byte_string_literal(&interface.name);
    let version_value = Literal::i32_unsuffixed(interface.version as i32);
    let request_count_value = Literal::i32_unsuffixed(interface.requests.len() as i32);
    let requests_value = if interface.requests.is_empty() {
        quote! { null::<wayland_backend::protocol::wl_message>() }
    } else {
        let requests_ident = format_ident!("{}_requests", interface.name);
        quote! { #requests_ident.0.as_ptr() }
    };
    let event_count_value = Literal::i32_unsuffixed(interface.events.len() as i32);
    let events_value = if interface.events.is_empty() {
        quote! { null::<wayland_backend::protocol::wl_message>() }
    } else {
        let events_ident = format_ident!("{}_events", interface.name);
        quote! { #events_ident.0.as_ptr() }
    };

    quote! {
        #requests
        #events

        pub static #interface_ident: wayland_backend::protocol::wl_interface = wayland_backend::protocol::wl_interface {
            name: #name_value as *const u8 as *const std::os::raw::c_char,
            version: #version_value,
            request_count: #request_count_value,
            requests: #requests_value,
            event_count: #event_count_value,
            events: #events_value,
        };
    }
}

fn gen_messages(interface: &Interface, messages: &[Message], which: &str) -> TokenStream {
    if messages.is_empty() {
        return TokenStream::new();
    }

    let types_arrays = messages.iter().filter_map(|msg| {
        if msg.all_null() {
            None
        } else {
            let array_ident = format_ident!("{}_{}_{}_types", interface.name, which, msg.name);
            let array_len = Literal::usize_unsuffixed(msg.args.len());
            let array_values = msg.args.iter().map(|arg| match (arg.typ, &arg.interface) {
                (Type::Object, &Some(ref inter)) | (Type::NewId, &Some(ref inter)) => {
                    let interface_ident =format_ident!("{}_interface", inter);
                    quote! { &#interface_ident as *const wayland_backend::protocol::wl_interface }
                }
                _ => quote! { null::<wayland_backend::protocol::wl_interface>() },
            });

            Some(quote! {
                static #array_ident: SyncWrapper<[*const wayland_backend::protocol::wl_interface; #array_len]> = SyncWrapper([
                    #(#array_values,)*
                ]);
            })
        }
    });

    let message_array_ident = format_ident!("{}_{}", interface.name, which);
    let message_array_len = Literal::usize_unsuffixed(messages.len());
    let message_array_values = messages.iter().map(|msg| {
        let name_value = null_terminated_byte_string_literal(&msg.name);
        let signature_value = Literal::byte_string(&message_signature(msg));

        let types_ident = if msg.all_null() {
            format_ident!("types_null")
        } else {
            format_ident!("{}_{}_{}_types", interface.name, which, msg.name)
        };

        quote! {
            wayland_backend::protocol::wl_message {
                name: #name_value as *const u8 as *const std::os::raw::c_char,
                signature: #signature_value as *const u8 as *const std::os::raw::c_char,
                types: #types_ident.0.as_ptr(),
            }
        }
    });

    quote! {
        #(#types_arrays)*

        static #message_array_ident: SyncWrapper<[wayland_backend::protocol::wl_message; #message_array_len]> = SyncWrapper([
            #(#message_array_values,)*
        ]);
    }
}

fn message_signature(msg: &Message) -> Vec<u8> {
    let mut res = Vec::new();

    if msg.since > 1 {
        res.extend_from_slice(msg.since.to_string().as_bytes());
    }

    for arg in &msg.args {
        if arg.typ.nullable() && arg.allow_null {
            res.push(b'?');
        }
        match arg.typ {
            Type::NewId => {
                if arg.interface.is_none() {
                    res.extend_from_slice(b"su");
                }
                res.push(b'n');
            }
            Type::Uint => res.push(b'u'),
            Type::Fixed => res.push(b'f'),
            Type::String => res.push(b's'),
            Type::Object => res.push(b'o'),
            Type::Array => res.push(b'a'),
            Type::Fd => res.push(b'h'),
            Type::Int => res.push(b'i'),
            _ => {}
        }
    }

    res.push(0);
    res
}

pub fn null_terminated_byte_string_literal(string: &str) -> Literal {
    let mut val = Vec::with_capacity(string.len() + 1);
    val.extend_from_slice(string.as_bytes());
    val.push(0);

    Literal::byte_string(&val)
}
