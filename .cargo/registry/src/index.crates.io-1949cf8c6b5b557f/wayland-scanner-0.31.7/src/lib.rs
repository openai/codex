//! Wayland protocol code-generation machinnery
//!
//! This crate provides procedural macros for generating the rust code associated with a
//! Wayland XML protocol specification, for use with the `wayland-client`, `wayland-server`
//! and `wayland-backend` crates.
//!
//! Before trying to use this crate, you may check if the protocol extension you want to use
//! is not already exposed in the `wayland-protocols` crate.
//!
//! ## Example usage
//!
//! Below is a template for generating the code for a custom protocol client-side. Server-side
//! is identical, just replacing `client` by `server`. The path to the XML file is relative to the
//! crate root.
//!
//! ```rust,ignore
//! // Generate the bindings in their own module
//! pub mod my_protocol {
//!     use wayland_client;
//!     // import objects from the core protocol if needed
//!     use wayland_client::protocol::*;
//!
//!     // This module hosts a low-level representation of the protocol objects
//!     // you will not need to interact with it yourself, but the code generated
//!     // by the generate_client_code! macro will use it
//!     pub mod __interfaces {
//!         // import the interfaces from the core protocol if needed
//!         use wayland_client::protocol::__interfaces::*;
//!         wayland_scanner::generate_interfaces!("./path/to/the/protocol.xml");
//!     }
//!     use self::__interfaces::*;
//!
//!     // This macro generates the actual types that represent the wayland objects of
//!     // your custom protocol
//!     wayland_scanner::generate_client_code!("./path/to/the/protocol.xml");
//! }
//! ```

use std::{ffi::OsString, path::PathBuf};

mod c_interfaces;
mod client_gen;
mod common;
mod interfaces;
mod parse;
mod protocol;
mod server_gen;
mod token;
mod util;

/// Proc-macro for generating low-level interfaces associated with an XML specification
#[proc_macro]
pub fn generate_interfaces(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path: OsString = token::parse_lit_str_token(stream).into();
    let path = if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let mut buf = PathBuf::from(manifest_dir);
        buf.push(path);
        buf
    } else {
        path.into()
    };
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(e) => panic!("Failed to open protocol file {}: {}", path.display(), e),
    };
    let protocol = parse::parse(file);
    interfaces::generate(&protocol, true).into()
}

/// Proc-macro for generating client-side API associated with an XML specification
#[proc_macro]
pub fn generate_client_code(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path: OsString = token::parse_lit_str_token(stream).into();
    let path = if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let mut buf = PathBuf::from(manifest_dir);
        buf.push(path);
        buf
    } else {
        path.into()
    };
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(e) => panic!("Failed to open protocol file {}: {}", path.display(), e),
    };
    let protocol = parse::parse(file);
    client_gen::generate_client_objects(&protocol).into()
}

/// Proc-macro for generating server-side API associated with an XML specification
#[proc_macro]
pub fn generate_server_code(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path: OsString = token::parse_lit_str_token(stream).into();
    let path = if let Some(manifest_dir) = std::env::var_os("CARGO_MANIFEST_DIR") {
        let mut buf = PathBuf::from(manifest_dir);
        buf.push(path);
        buf
    } else {
        path.into()
    };
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(e) => panic!("Failed to open protocol file {}: {}", path.display(), e),
    };
    let protocol = parse::parse(file);
    server_gen::generate_server_objects(&protocol).into()
}

#[cfg(test)]
fn format_rust_code(code: &str) -> String {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    if let Ok(mut proc) = Command::new("rustfmt")
        .arg("--emit=stdout")
        .arg("--edition=2018")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        //.stderr(Stdio::null())
        .spawn()
    {
        {
            let stdin = proc.stdin.as_mut().unwrap();
            stdin.write_all(code.as_bytes()).unwrap();
        }
        if let Ok(output) = proc.wait_with_output() {
            if output.status.success() {
                return std::str::from_utf8(&output.stdout).unwrap().to_owned();
            }
        }
    }
    panic!("Rustfmt failed!");
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Side {
    /// wayland client applications
    Client,
    /// wayland compositors
    Server,
}
