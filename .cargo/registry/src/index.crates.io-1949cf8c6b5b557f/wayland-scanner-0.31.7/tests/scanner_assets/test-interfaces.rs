use std::ptr::null;
struct SyncWrapper<T>(T);
unsafe impl<T> Sync for SyncWrapper<T> {}
static types_null: SyncWrapper<[*const wayland_backend::protocol::wl_interface; 6]> =
    SyncWrapper([null::<wayland_backend::protocol::wl_interface>(); 6]);
pub static WL_DISPLAY_INTERFACE: wayland_backend::protocol::Interface =
    wayland_backend::protocol::Interface {
        name: "wl_display",
        version: 1u32,
        requests: &[
            wayland_backend::protocol::MessageDesc {
                name: "sync",
                signature: &[wayland_backend::protocol::ArgumentType::NewId],
                since: 1u32,
                is_destructor: false,
                child_interface: Some(&WL_CALLBACK_INTERFACE),
                arg_interfaces: &[],
            },
            wayland_backend::protocol::MessageDesc {
                name: "get_registry",
                signature: &[wayland_backend::protocol::ArgumentType::NewId],
                since: 1u32,
                is_destructor: false,
                child_interface: Some(&WL_REGISTRY_INTERFACE),
                arg_interfaces: &[],
            },
        ],
        events: &[
            wayland_backend::protocol::MessageDesc {
                name: "error",
                signature: &[
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                    wayland_backend::protocol::ArgumentType::Uint,
                    wayland_backend::protocol::ArgumentType::Str(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                ],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[&wayland_backend::protocol::ANONYMOUS_INTERFACE],
            },
            wayland_backend::protocol::MessageDesc {
                name: "delete_id",
                signature: &[wayland_backend::protocol::ArgumentType::Uint],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[],
            },
        ],
        c_ptr: Some(unsafe { &wl_display_interface }),
    };
static wl_display_requests_sync_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 1],
> = SyncWrapper([&wl_callback_interface as *const wayland_backend::protocol::wl_interface]);
static wl_display_requests_get_registry_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 1],
> = SyncWrapper([&wl_registry_interface as *const wayland_backend::protocol::wl_interface]);
static wl_display_requests: SyncWrapper<[wayland_backend::protocol::wl_message; 2]> =
    SyncWrapper([
        wayland_backend::protocol::wl_message {
            name: b"sync\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"n\0" as *const u8 as *const std::os::raw::c_char,
            types: wl_display_requests_sync_types.0.as_ptr(),
        },
        wayland_backend::protocol::wl_message {
            name: b"get_registry\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"n\0" as *const u8 as *const std::os::raw::c_char,
            types: wl_display_requests_get_registry_types.0.as_ptr(),
        },
    ]);
static wl_display_events: SyncWrapper<[wayland_backend::protocol::wl_message; 2]> = SyncWrapper([
    wayland_backend::protocol::wl_message {
        name: b"error\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"ous\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    },
    wayland_backend::protocol::wl_message {
        name: b"delete_id\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"u\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    },
]);
pub static wl_display_interface: wayland_backend::protocol::wl_interface =
    wayland_backend::protocol::wl_interface {
        name: b"wl_display\0" as *const u8 as *const std::os::raw::c_char,
        version: 1,
        request_count: 2,
        requests: wl_display_requests.0.as_ptr(),
        event_count: 2,
        events: wl_display_events.0.as_ptr(),
    };
pub static WL_REGISTRY_INTERFACE: wayland_backend::protocol::Interface =
    wayland_backend::protocol::Interface {
        name: "wl_registry",
        version: 1u32,
        requests: &[wayland_backend::protocol::MessageDesc {
            name: "bind",
            signature: &[
                wayland_backend::protocol::ArgumentType::Uint,
                wayland_backend::protocol::ArgumentType::Str(
                    wayland_backend::protocol::AllowNull::No,
                ),
                wayland_backend::protocol::ArgumentType::Uint,
                wayland_backend::protocol::ArgumentType::NewId,
            ],
            since: 1u32,
            is_destructor: false,
            child_interface: None,
            arg_interfaces: &[],
        }],
        events: &[
            wayland_backend::protocol::MessageDesc {
                name: "global",
                signature: &[
                    wayland_backend::protocol::ArgumentType::Uint,
                    wayland_backend::protocol::ArgumentType::Str(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                    wayland_backend::protocol::ArgumentType::Uint,
                ],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[],
            },
            wayland_backend::protocol::MessageDesc {
                name: "global_remove",
                signature: &[wayland_backend::protocol::ArgumentType::Uint],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[],
            },
        ],
        c_ptr: Some(unsafe { &wl_registry_interface }),
    };
static wl_registry_requests: SyncWrapper<[wayland_backend::protocol::wl_message; 1]> =
    SyncWrapper([wayland_backend::protocol::wl_message {
        name: b"bind\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"usun\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    }]);
static wl_registry_events: SyncWrapper<[wayland_backend::protocol::wl_message; 2]> = SyncWrapper([
    wayland_backend::protocol::wl_message {
        name: b"global\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"usu\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    },
    wayland_backend::protocol::wl_message {
        name: b"global_remove\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"u\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    },
]);
pub static wl_registry_interface: wayland_backend::protocol::wl_interface =
    wayland_backend::protocol::wl_interface {
        name: b"wl_registry\0" as *const u8 as *const std::os::raw::c_char,
        version: 1,
        request_count: 1,
        requests: wl_registry_requests.0.as_ptr(),
        event_count: 2,
        events: wl_registry_events.0.as_ptr(),
    };
pub static WL_CALLBACK_INTERFACE: wayland_backend::protocol::Interface =
    wayland_backend::protocol::Interface {
        name: "wl_callback",
        version: 1u32,
        requests: &[],
        events: &[wayland_backend::protocol::MessageDesc {
            name: "done",
            signature: &[wayland_backend::protocol::ArgumentType::Uint],
            since: 1u32,
            is_destructor: true,
            child_interface: None,
            arg_interfaces: &[],
        }],
        c_ptr: Some(unsafe { &wl_callback_interface }),
    };
static wl_callback_events: SyncWrapper<[wayland_backend::protocol::wl_message; 1]> =
    SyncWrapper([wayland_backend::protocol::wl_message {
        name: b"done\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"u\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    }]);
pub static wl_callback_interface: wayland_backend::protocol::wl_interface =
    wayland_backend::protocol::wl_interface {
        name: b"wl_callback\0" as *const u8 as *const std::os::raw::c_char,
        version: 1,
        request_count: 0,
        requests: null::<wayland_backend::protocol::wl_message>(),
        event_count: 1,
        events: wl_callback_events.0.as_ptr(),
    };
pub static TEST_GLOBAL_INTERFACE: wayland_backend::protocol::Interface =
    wayland_backend::protocol::Interface {
        name: "test_global",
        version: 5u32,
        requests: &[
            wayland_backend::protocol::MessageDesc {
                name: "many_args",
                signature: &[
                    wayland_backend::protocol::ArgumentType::Uint,
                    wayland_backend::protocol::ArgumentType::Int,
                    wayland_backend::protocol::ArgumentType::Fixed,
                    wayland_backend::protocol::ArgumentType::Array,
                    wayland_backend::protocol::ArgumentType::Str(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                    wayland_backend::protocol::ArgumentType::Fd,
                ],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[],
            },
            wayland_backend::protocol::MessageDesc {
                name: "get_secondary",
                signature: &[wayland_backend::protocol::ArgumentType::NewId],
                since: 2u32,
                is_destructor: false,
                child_interface: Some(&SECONDARY_INTERFACE),
                arg_interfaces: &[],
            },
            wayland_backend::protocol::MessageDesc {
                name: "get_tertiary",
                signature: &[wayland_backend::protocol::ArgumentType::NewId],
                since: 3u32,
                is_destructor: false,
                child_interface: Some(&TERTIARY_INTERFACE),
                arg_interfaces: &[],
            },
            wayland_backend::protocol::MessageDesc {
                name: "link",
                signature: &[
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::Yes,
                    ),
                    wayland_backend::protocol::ArgumentType::Uint,
                ],
                since: 3u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[&SECONDARY_INTERFACE, &TERTIARY_INTERFACE],
            },
            wayland_backend::protocol::MessageDesc {
                name: "destroy",
                signature: &[],
                since: 4u32,
                is_destructor: true,
                child_interface: None,
                arg_interfaces: &[],
            },
            wayland_backend::protocol::MessageDesc {
                name: "reverse_link",
                signature: &[
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::Yes,
                    ),
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                ],
                since: 5u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[&SECONDARY_INTERFACE, &TERTIARY_INTERFACE],
            },
            wayland_backend::protocol::MessageDesc {
                name: "newid_and_allow_null",
                signature: &[
                    wayland_backend::protocol::ArgumentType::NewId,
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::Yes,
                    ),
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                ],
                since: 5u32,
                is_destructor: false,
                child_interface: Some(&QUAD_INTERFACE),
                arg_interfaces: &[&SECONDARY_INTERFACE, &TERTIARY_INTERFACE],
            },
        ],
        events: &[
            wayland_backend::protocol::MessageDesc {
                name: "many_args_evt",
                signature: &[
                    wayland_backend::protocol::ArgumentType::Uint,
                    wayland_backend::protocol::ArgumentType::Int,
                    wayland_backend::protocol::ArgumentType::Fixed,
                    wayland_backend::protocol::ArgumentType::Array,
                    wayland_backend::protocol::ArgumentType::Str(
                        wayland_backend::protocol::AllowNull::No,
                    ),
                    wayland_backend::protocol::ArgumentType::Fd,
                ],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[],
            },
            wayland_backend::protocol::MessageDesc {
                name: "ack_secondary",
                signature: &[wayland_backend::protocol::ArgumentType::Object(
                    wayland_backend::protocol::AllowNull::No,
                )],
                since: 1u32,
                is_destructor: false,
                child_interface: None,
                arg_interfaces: &[&SECONDARY_INTERFACE],
            },
            wayland_backend::protocol::MessageDesc {
                name: "cycle_quad",
                signature: &[
                    wayland_backend::protocol::ArgumentType::NewId,
                    wayland_backend::protocol::ArgumentType::Object(
                        wayland_backend::protocol::AllowNull::Yes,
                    ),
                ],
                since: 1u32,
                is_destructor: false,
                child_interface: Some(&QUAD_INTERFACE),
                arg_interfaces: &[&QUAD_INTERFACE],
            },
        ],
        c_ptr: Some(unsafe { &test_global_interface }),
    };
static test_global_requests_get_secondary_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 1],
> = SyncWrapper([&secondary_interface as *const wayland_backend::protocol::wl_interface]);
static test_global_requests_get_tertiary_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 1],
> = SyncWrapper([&tertiary_interface as *const wayland_backend::protocol::wl_interface]);
static test_global_requests_link_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 3],
> = SyncWrapper([
    &secondary_interface as *const wayland_backend::protocol::wl_interface,
    &tertiary_interface as *const wayland_backend::protocol::wl_interface,
    null::<wayland_backend::protocol::wl_interface>(),
]);
static test_global_requests_reverse_link_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 2],
> = SyncWrapper([
    &secondary_interface as *const wayland_backend::protocol::wl_interface,
    &tertiary_interface as *const wayland_backend::protocol::wl_interface,
]);
static test_global_requests_newid_and_allow_null_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 3],
> = SyncWrapper([
    &quad_interface as *const wayland_backend::protocol::wl_interface,
    &secondary_interface as *const wayland_backend::protocol::wl_interface,
    &tertiary_interface as *const wayland_backend::protocol::wl_interface,
]);
static test_global_requests: SyncWrapper<[wayland_backend::protocol::wl_message; 7]> =
    SyncWrapper([
        wayland_backend::protocol::wl_message {
            name: b"many_args\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"uifash\0" as *const u8 as *const std::os::raw::c_char,
            types: types_null.0.as_ptr(),
        },
        wayland_backend::protocol::wl_message {
            name: b"get_secondary\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"2n\0" as *const u8 as *const std::os::raw::c_char,
            types: test_global_requests_get_secondary_types.0.as_ptr(),
        },
        wayland_backend::protocol::wl_message {
            name: b"get_tertiary\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"3n\0" as *const u8 as *const std::os::raw::c_char,
            types: test_global_requests_get_tertiary_types.0.as_ptr(),
        },
        wayland_backend::protocol::wl_message {
            name: b"link\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"3o?ou\0" as *const u8 as *const std::os::raw::c_char,
            types: test_global_requests_link_types.0.as_ptr(),
        },
        wayland_backend::protocol::wl_message {
            name: b"destroy\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"4\0" as *const u8 as *const std::os::raw::c_char,
            types: types_null.0.as_ptr(),
        },
        wayland_backend::protocol::wl_message {
            name: b"reverse_link\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"5?oo\0" as *const u8 as *const std::os::raw::c_char,
            types: test_global_requests_reverse_link_types.0.as_ptr(),
        },
        wayland_backend::protocol::wl_message {
            name: b"newid_and_allow_null\0" as *const u8 as *const std::os::raw::c_char,
            signature: b"5n?oo\0" as *const u8 as *const std::os::raw::c_char,
            types: test_global_requests_newid_and_allow_null_types.0.as_ptr(),
        },
    ]);
static test_global_events_ack_secondary_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 1],
> = SyncWrapper([&secondary_interface as *const wayland_backend::protocol::wl_interface]);
static test_global_events_cycle_quad_types: SyncWrapper<
    [*const wayland_backend::protocol::wl_interface; 2],
> = SyncWrapper([
    &quad_interface as *const wayland_backend::protocol::wl_interface,
    &quad_interface as *const wayland_backend::protocol::wl_interface,
]);
static test_global_events: SyncWrapper<[wayland_backend::protocol::wl_message; 3]> = SyncWrapper([
    wayland_backend::protocol::wl_message {
        name: b"many_args_evt\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"uifash\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    },
    wayland_backend::protocol::wl_message {
        name: b"ack_secondary\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"o\0" as *const u8 as *const std::os::raw::c_char,
        types: test_global_events_ack_secondary_types.0.as_ptr(),
    },
    wayland_backend::protocol::wl_message {
        name: b"cycle_quad\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"n?o\0" as *const u8 as *const std::os::raw::c_char,
        types: test_global_events_cycle_quad_types.0.as_ptr(),
    },
]);
pub static test_global_interface: wayland_backend::protocol::wl_interface =
    wayland_backend::protocol::wl_interface {
        name: b"test_global\0" as *const u8 as *const std::os::raw::c_char,
        version: 5,
        request_count: 7,
        requests: test_global_requests.0.as_ptr(),
        event_count: 3,
        events: test_global_events.0.as_ptr(),
    };
pub static SECONDARY_INTERFACE: wayland_backend::protocol::Interface =
    wayland_backend::protocol::Interface {
        name: "secondary",
        version: 5u32,
        requests: &[wayland_backend::protocol::MessageDesc {
            name: "destroy",
            signature: &[],
            since: 2u32,
            is_destructor: true,
            child_interface: None,
            arg_interfaces: &[],
        }],
        events: &[],
        c_ptr: Some(unsafe { &secondary_interface }),
    };
static secondary_requests: SyncWrapper<[wayland_backend::protocol::wl_message; 1]> =
    SyncWrapper([wayland_backend::protocol::wl_message {
        name: b"destroy\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"2\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    }]);
pub static secondary_interface: wayland_backend::protocol::wl_interface =
    wayland_backend::protocol::wl_interface {
        name: b"secondary\0" as *const u8 as *const std::os::raw::c_char,
        version: 5,
        request_count: 1,
        requests: secondary_requests.0.as_ptr(),
        event_count: 0,
        events: null::<wayland_backend::protocol::wl_message>(),
    };
pub static TERTIARY_INTERFACE: wayland_backend::protocol::Interface =
    wayland_backend::protocol::Interface {
        name: "tertiary",
        version: 5u32,
        requests: &[wayland_backend::protocol::MessageDesc {
            name: "destroy",
            signature: &[],
            since: 3u32,
            is_destructor: true,
            child_interface: None,
            arg_interfaces: &[],
        }],
        events: &[],
        c_ptr: Some(unsafe { &tertiary_interface }),
    };
static tertiary_requests: SyncWrapper<[wayland_backend::protocol::wl_message; 1]> =
    SyncWrapper([wayland_backend::protocol::wl_message {
        name: b"destroy\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"3\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    }]);
pub static tertiary_interface: wayland_backend::protocol::wl_interface =
    wayland_backend::protocol::wl_interface {
        name: b"tertiary\0" as *const u8 as *const std::os::raw::c_char,
        version: 5,
        request_count: 1,
        requests: tertiary_requests.0.as_ptr(),
        event_count: 0,
        events: null::<wayland_backend::protocol::wl_message>(),
    };
pub static QUAD_INTERFACE: wayland_backend::protocol::Interface =
    wayland_backend::protocol::Interface {
        name: "quad",
        version: 5u32,
        requests: &[wayland_backend::protocol::MessageDesc {
            name: "destroy",
            signature: &[],
            since: 3u32,
            is_destructor: true,
            child_interface: None,
            arg_interfaces: &[],
        }],
        events: &[],
        c_ptr: Some(unsafe { &quad_interface }),
    };
static quad_requests: SyncWrapper<[wayland_backend::protocol::wl_message; 1]> =
    SyncWrapper([wayland_backend::protocol::wl_message {
        name: b"destroy\0" as *const u8 as *const std::os::raw::c_char,
        signature: b"3\0" as *const u8 as *const std::os::raw::c_char,
        types: types_null.0.as_ptr(),
    }]);
pub static quad_interface: wayland_backend::protocol::wl_interface =
    wayland_backend::protocol::wl_interface {
        name: b"quad\0" as *const u8 as *const std::os::raw::c_char,
        version: 5,
        request_count: 1,
        requests: quad_requests.0.as_ptr(),
        event_count: 0,
        events: null::<wayland_backend::protocol::wl_message>(),
    };
