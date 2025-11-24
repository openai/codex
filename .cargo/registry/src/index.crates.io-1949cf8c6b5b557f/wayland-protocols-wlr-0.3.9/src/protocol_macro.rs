macro_rules! wayland_protocol(
    ($path:expr, [$($imports:path),*]) => {
        #[cfg(feature = "client")]
        pub use self::generated::client;

        #[cfg(feature = "server")]
        pub use self::generated::server;

        mod generated {
            #![allow(dead_code,non_camel_case_types,unused_unsafe,unused_variables)]
            #![allow(non_upper_case_globals,non_snake_case,unused_imports)]
            #![allow(missing_docs, clippy::all)]

            #[cfg(feature = "client")]
            pub mod client {
                //! Client-side API of this protocol
                use wayland_client;
                use wayland_client::protocol::*;
                $(use $imports::{client::*};)*

                pub mod __interfaces {
                    use wayland_client::protocol::__interfaces::*;
                    $(use $imports::{client::__interfaces::*};)*
                    wayland_scanner::generate_interfaces!($path);
                }
                use self::__interfaces::*;

                wayland_scanner::generate_client_code!($path);
            }

            #[cfg(feature = "server")]
            pub mod server {
                //! Server-side API of this protocol
                use wayland_server;
                use wayland_server::protocol::*;
                $(use $imports::{server::*};)*

                pub mod __interfaces {
                    use wayland_server::protocol::__interfaces::*;
                    $(use $imports::{server::__interfaces::*};)*
                    wayland_scanner::generate_interfaces!($path);
                }
                use self::__interfaces::*;

                wayland_scanner::generate_server_code!($path);
            }
        }
    }
);
