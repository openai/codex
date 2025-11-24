#[cfg(not(target_env = "musl"))]
use crate::errno::Errno;
use crate::sys::signal::SigSet;
#[cfg(not(target_env = "musl"))]
use crate::Result;
#[cfg(not(target_env = "musl"))]
use std::mem;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UContext {
    context: libc::ucontext_t,
}

impl UContext {
    #[cfg(not(target_env = "musl"))]
    pub fn get() -> Result<UContext> {
        let mut context = mem::MaybeUninit::<libc::ucontext_t>::uninit();
        let res = unsafe { libc::getcontext(context.as_mut_ptr()) };
        Errno::result(res).map(|_| unsafe {
            UContext {
                context: context.assume_init(),
            }
        })
    }

    #[cfg(not(target_env = "musl"))]
    pub fn set(&self) -> Result<()> {
        let res = unsafe {
            libc::setcontext(&self.context as *const libc::ucontext_t)
        };
        Errno::result(res).map(drop)
    }

    pub fn sigmask_mut(&mut self) -> &mut SigSet {
        unsafe {
            &mut *(&mut self.context.uc_sigmask as *mut libc::sigset_t
                as *mut SigSet)
        }
    }

    pub fn sigmask(&self) -> &SigSet {
        unsafe {
            &*(&self.context.uc_sigmask as *const libc::sigset_t
                as *const SigSet)
        }
    }
}
