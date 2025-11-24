// This code exercises the surface area that we expect of the Error generic
// member access API. If the current toolchain is able to compile it, then
// anyhow is able to provide backtrace support.

#![cfg_attr(anyhow_build_probe, feature(error_generic_member_access))]

use core::error::{self, Error};
use std::backtrace::Backtrace;

pub use core::error::Request;

#[cfg(anyhow_build_probe)]
const _: () = {
    use core::fmt::{self, Debug, Display};

    struct MyError(Backtrace);

    impl Debug for MyError {
        fn fmt(&self, _formatter: &mut fmt::Formatter) -> fmt::Result {
            unimplemented!()
        }
    }

    impl Display for MyError {
        fn fmt(&self, _formatter: &mut fmt::Formatter) -> fmt::Result {
            unimplemented!()
        }
    }

    impl Error for MyError {
        fn provide<'a>(&'a self, request: &mut Request<'a>) {
            provide_ref_backtrace(request, &self.0);
        }
    }
};

// Include in sccache cache key.
#[cfg(anyhow_build_probe)]
const _: Option<&str> = option_env!("RUSTC_BOOTSTRAP");

pub fn request_ref_backtrace(err: &dyn Error) -> Option<&Backtrace> {
    request_ref::<Backtrace>(err)
}

fn request_ref<'a, T>(err: &'a (impl Error + ?Sized)) -> Option<&'a T>
where
    T: 'static + ?Sized,
{
    error::request_ref::<T>(err)
}

pub fn provide_ref_backtrace<'a>(request: &mut Request<'a>, backtrace: &'a Backtrace) {
    Request::provide_ref(request, backtrace);
}

pub fn provide<'a>(err: &'a (impl Error + ?Sized), request: &mut Request<'a>) {
    Error::provide(err, request);
}
