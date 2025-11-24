//! Core interfaces of the protocol
//!
//! This module contains hard-coded interfaces for `wl_display`, `wl_registry` and `wl_callback`.
//! These interfaces are frozen in the protocol and can never change. They are the only interfaces
//! which the backends need to be aware of in particular.

use crate::protocol::{AllowNull, ArgumentType, Interface, MessageDesc, ANONYMOUS_INTERFACE};

/// Interface `wl_display`
pub static WL_DISPLAY_INTERFACE: Interface = Interface {
    name: "wl_display",
    version: 1,
    requests: &[
        MessageDesc {
            name: "sync",
            since: 1,
            is_destructor: false,
            signature: &[ArgumentType::NewId],
            child_interface: Some(&WL_CALLBACK_INTERFACE),
            arg_interfaces: &[],
        },
        MessageDesc {
            name: "get_registry",
            since: 1,
            is_destructor: false,
            signature: &[ArgumentType::NewId],
            child_interface: Some(&WL_REGISTRY_INTERFACE),
            arg_interfaces: &[],
        },
    ],
    events: &[
        MessageDesc {
            name: "error",
            since: 1,
            is_destructor: false,
            signature: &[
                ArgumentType::Object(AllowNull::No),
                ArgumentType::Uint,
                ArgumentType::Str(AllowNull::No),
            ],
            child_interface: None,
            arg_interfaces: &[&ANONYMOUS_INTERFACE],
        },
        MessageDesc {
            name: "delete_id",
            since: 1,
            is_destructor: false,
            signature: &[ArgumentType::Uint],
            child_interface: None,
            arg_interfaces: &[],
        },
    ],
    c_ptr: None,
};

/// Interface `wl_registry`
pub static WL_REGISTRY_INTERFACE: Interface = Interface {
    name: "wl_registry",
    version: 1,
    requests: &[MessageDesc {
        name: "bind",
        since: 1,
        is_destructor: false,
        signature: &[
            ArgumentType::Uint,
            ArgumentType::Str(AllowNull::No),
            ArgumentType::Uint,
            ArgumentType::NewId,
        ],
        child_interface: None,
        arg_interfaces: &[],
    }],
    events: &[
        MessageDesc {
            name: "global",
            since: 1,
            is_destructor: false,
            signature: &[ArgumentType::Uint, ArgumentType::Str(AllowNull::No), ArgumentType::Uint],
            child_interface: None,
            arg_interfaces: &[],
        },
        MessageDesc {
            name: "global_remove",
            since: 1,
            is_destructor: false,
            signature: &[ArgumentType::Uint],
            child_interface: None,
            arg_interfaces: &[],
        },
    ],
    c_ptr: None,
};

/// Interface `wl_callback`
pub static WL_CALLBACK_INTERFACE: Interface = Interface {
    name: "wl_callback",
    version: 1,
    requests: &[],
    events: &[MessageDesc {
        name: "done",
        since: 1,
        is_destructor: true,
        signature: &[ArgumentType::Uint],
        child_interface: None,
        arg_interfaces: &[],
    }],
    c_ptr: None,
};
