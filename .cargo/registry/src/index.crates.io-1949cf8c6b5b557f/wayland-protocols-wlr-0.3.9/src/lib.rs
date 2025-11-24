//! This crate provides bindings to the wlroots wayland protocol extensions
//! provided in <https://gitlab.freedesktop.org/wlroots/wlr-protocols>
//!
//! These bindings are built on top of the crates wayland-client and wayland-server.
//!
//! Each protocol module contains a `client` and a `server` submodules, for each side of the
//! protocol. The creation of these modules (and the dependency on the associated crate) is
//! controlled by the two cargo features `client` and `server`.

#![warn(missing_docs)]
#![forbid(improper_ctypes, unsafe_op_in_unsafe_fn)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(rustfmt, rustfmt_skip)]

#[macro_use]
mod protocol_macro;

pub mod data_control {
    //! Control data devices, particularly the clipboard.
    //!
    //! An interface to control data devices, particularly to manage the current selection and
    //! take the role of a clipboard manager.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-data-control-unstable-v1.xml",
            []
        );
    }
}

pub mod export_dmabuf {
    //! A protocol for low overhead screen content capturing
    //!
    //! An interface to capture surfaces in an efficient way by exporting DMA-BUFs.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-export-dmabuf-unstable-v1.xml",
            []
        );
    }
}

pub mod foreign_toplevel {
    //! List and control opened apps
    //!
    //! Use for creating taskbars and docks.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-foreign-toplevel-management-unstable-v1.xml",
            []
        );
    }
}

pub mod gamma_control {
    //! Manage gamma tables of outputs.
    //!
    //! This protocol allows a privileged client to set the gamma tables for outputs.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-gamma-control-unstable-v1.xml",
            []
        );
    }
}

pub mod input_inhibitor {
    //! Inhibits input events to other clients

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-input-inhibitor-unstable-v1.xml",
            []
        );
    }
}

pub mod layer_shell {
    //! Layered shell protocol

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-layer-shell-unstable-v1.xml",
            [wayland_protocols::xdg::shell]
        );
    }
}

pub mod output_management {
    //! Output management protocol
    //!
    //! This protocol exposes interfaces to obtain and modify output device configuration.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-output-management-unstable-v1.xml",
            []
        );
    }
}

pub mod output_power_management {
    //! Output power management protocol
    //!
    //! This protocol allows clients to control power management modes
    //! of outputs that are currently part of the compositor space. The
    //! intent is to allow special clients like desktop shells to power
    //! down outputs when the system is idle.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-output-power-management-unstable-v1.xml",
            []
        );
    }
}

pub mod screencopy {
    //! Screen content capturing on client buffers
    //!
    //! This protocol allows clients to ask the compositor to copy part of the
    //! screen content to a client buffer.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-screencopy-unstable-v1.xml",
            []
        );
    }
}

pub mod virtual_pointer {
    //! Virtual pointer protocol
    //!
    //! This protocol allows clients to emulate a physical pointer device. The
    //! requests are mostly mirror opposites of those specified in wl_pointer.

    #[allow(missing_docs)]
    pub mod v1 {
        wayland_protocol!(
            "./wlr-protocols/unstable/wlr-virtual-pointer-unstable-v1.xml",
            []
        );
    }
}