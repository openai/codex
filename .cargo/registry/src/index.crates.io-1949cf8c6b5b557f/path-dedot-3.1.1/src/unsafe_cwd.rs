use std::{env, ops::Deref, path::PathBuf};

/// Current working directory.
#[doc(hidden)]
pub struct UnsafeCWD {
    path: Option<PathBuf>,
}

impl UnsafeCWD {
    #[inline]
    pub(crate) const fn new() -> UnsafeCWD {
        UnsafeCWD {
            path: None
        }
    }

    #[inline]
    pub(crate) fn update(&mut self) {
        let cwd = env::current_dir().unwrap();

        self.path.replace(cwd);
    }

    #[inline]
    #[doc(hidden)]
    pub fn initial(&mut self) {
        if self.path.is_none() {
            self.update();
        }
    }
}

impl Deref for UnsafeCWD {
    type Target = PathBuf;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.path.as_ref().unwrap()
    }
}
