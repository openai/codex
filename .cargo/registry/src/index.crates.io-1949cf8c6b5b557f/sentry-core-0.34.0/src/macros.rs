/// Returns the intended release for Sentry as an `Option<Cow<'static, str>>`.
///
/// This can be used with `ClientOptions` to set the release name.  It uses
/// the information supplied by cargo to calculate a release.
///
/// # Examples
///
/// ```
/// # #[macro_use] extern crate sentry;
/// # fn main() {
/// let _sentry = sentry::init(sentry::ClientOptions {
///     release: sentry::release_name!(),
///     ..Default::default()
/// });
/// # }
/// ```
#[macro_export]
macro_rules! release_name {
    () => {{
        use std::sync::Once;
        static mut INIT: Once = Once::new();
        static mut RELEASE: Option<String> = None;
        unsafe {
            INIT.call_once(|| {
                RELEASE = option_env!("CARGO_PKG_NAME").and_then(|name| {
                    option_env!("CARGO_PKG_VERSION").map(|version| format!("{}@{}", name, version))
                });
            });
            RELEASE.as_ref().map(|x| {
                let release: &'static str = ::std::mem::transmute(x.as_str());
                ::std::borrow::Cow::Borrowed(release)
            })
        }
    }};
}

// TODO: temporarily exported for use in `sentry` crate
#[macro_export]
#[doc(hidden)]
macro_rules! with_client_impl {
    ($body:block) => {
        #[cfg(feature = "client")]
        {
            $body
        }
        #[cfg(not(feature = "client"))]
        {
            Default::default()
        }
    };
}

// TODO: temporarily exported for use in `sentry` crate
#[macro_export]
#[doc(hidden)]
macro_rules! sentry_debug {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug-logs")] {
            ::log::debug!(target: "sentry", $($arg)*);
        }
        #[cfg(not(feature = "debug-logs"))] {
            $crate::Hub::with(|hub| {
                if hub.client().map_or(false, |c| c.options().debug) {
                    eprint!("[sentry] ");
                    eprintln!($($arg)*);
                }
            });
        }
    }
}

#[allow(unused_macros)]
macro_rules! minimal_unreachable {
    () => {
        panic!(
            "this code should not be reachable. It's stubbed out for minimal usage. \
             If you get this error this is a bug in the sentry minimal support"
        );
    };
}
