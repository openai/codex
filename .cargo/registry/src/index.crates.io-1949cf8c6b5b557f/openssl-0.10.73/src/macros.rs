macro_rules! private_key_from_pem {
    ($(#[$m:meta])* $n:ident, $(#[$m2:meta])* $n2:ident, $(#[$m3:meta])* $n3:ident, $t:ty, $f:path) => {
        from_pem!($(#[$m])* $n, $t, $f);

        $(#[$m2])*
        pub fn $n2(pem: &[u8], passphrase: &[u8]) -> Result<$t, crate::error::ErrorStack> {
            unsafe {
                ffi::init();
                let bio = crate::bio::MemBioSlice::new(pem)?;
                let passphrase = ::std::ffi::CString::new(passphrase).unwrap();
                cvt_p($f(bio.as_ptr(),
                         ptr::null_mut(),
                         None,
                         passphrase.as_ptr() as *const _ as *mut _))
                    .map(|p| ::foreign_types::ForeignType::from_ptr(p))
            }
        }

        $(#[$m3])*
        pub fn $n3<F>(pem: &[u8], callback: F) -> Result<$t, crate::error::ErrorStack>
            where F: FnOnce(&mut [u8]) -> Result<usize, crate::error::ErrorStack>
        {
            unsafe {
                ffi::init();
                let mut cb = crate::util::CallbackState::new(callback);
                let bio = crate::bio::MemBioSlice::new(pem)?;
                cvt_p($f(bio.as_ptr(),
                         ptr::null_mut(),
                         Some(crate::util::invoke_passwd_cb::<F>),
                         &mut cb as *mut _ as *mut _))
                    .map(|p| ::foreign_types::ForeignType::from_ptr(p))
            }
        }
    }
}

macro_rules! private_key_to_pem {
    ($(#[$m:meta])* $n:ident, $(#[$m2:meta])* $n2:ident, $f:path) => {
        $(#[$m])*
        pub fn $n(&self) -> Result<Vec<u8>, crate::error::ErrorStack> {
            unsafe {
                let bio = crate::bio::MemBio::new()?;
                cvt($f(bio.as_ptr(),
                        self.as_ptr(),
                        ptr::null(),
                        ptr::null_mut(),
                        -1,
                        None,
                        ptr::null_mut()))?;
                Ok(bio.get_buf().to_owned())
            }
        }

        $(#[$m2])*
        pub fn $n2(
            &self,
            cipher: crate::symm::Cipher,
            passphrase: &[u8]
        ) -> Result<Vec<u8>, crate::error::ErrorStack> {
            unsafe {
                let bio = crate::bio::MemBio::new()?;
                assert!(passphrase.len() <= ::libc::c_int::MAX as usize);
                cvt($f(bio.as_ptr(),
                        self.as_ptr(),
                        cipher.as_ptr(),
                        passphrase.as_ptr() as *const _ as *mut _,
                        passphrase.len() as ::libc::c_int,
                        None,
                        ptr::null_mut()))?;
                Ok(bio.get_buf().to_owned())
            }
        }
    }
}

macro_rules! to_pem {
    ($(#[$m:meta])* $n:ident, $f:path) => {
        $(#[$m])*
        pub fn $n(&self) -> Result<Vec<u8>, crate::error::ErrorStack> {
            unsafe {
                let bio = crate::bio::MemBio::new()?;
                cvt($f(bio.as_ptr(), self.as_ptr()))?;
                Ok(bio.get_buf().to_owned())
            }
        }
    }
}

macro_rules! to_der {
    ($(#[$m:meta])* $n:ident, $f:path) => {
        $(#[$m])*
        pub fn $n(&self) -> Result<Vec<u8>, crate::error::ErrorStack> {
            unsafe {
                let len = crate::cvt($f(::foreign_types::ForeignTypeRef::as_ptr(self),
                                        ptr::null_mut()))?;
                let mut buf = vec![0; len as usize];
                crate::cvt($f(::foreign_types::ForeignTypeRef::as_ptr(self),
                              &mut buf.as_mut_ptr()))?;
                Ok(buf)
            }
        }
    };
}

macro_rules! from_der {
    ($(#[$m:meta])* $n:ident, $t:ty, $f:path) => {
        $(#[$m])*
        pub fn $n(der: &[u8]) -> Result<$t, crate::error::ErrorStack> {
            use std::convert::TryInto;
            unsafe {
                ffi::init();
                let len = ::std::cmp::min(der.len(), ::libc::c_long::MAX as usize) as ::libc::c_long;
                crate::cvt_p($f(::std::ptr::null_mut(), &mut der.as_ptr(), len.try_into().unwrap()))
                    .map(|p| ::foreign_types::ForeignType::from_ptr(p))
            }
        }
    }
}

macro_rules! from_pem {
    ($(#[$m:meta])* $n:ident, $t:ty, $f:path) => {
        $(#[$m])*
        pub fn $n(pem: &[u8]) -> Result<$t, crate::error::ErrorStack> {
            unsafe {
                crate::init();
                let bio = crate::bio::MemBioSlice::new(pem)?;
                cvt_p($f(bio.as_ptr(), ::std::ptr::null_mut(), None, ::std::ptr::null_mut()))
                    .map(|p| ::foreign_types::ForeignType::from_ptr(p))
            }
        }
    }
}

macro_rules! foreign_type_and_impl_send_sync {
    (
        $(#[$impl_attr:meta])*
        type CType = $ctype:ty;
        fn drop = $drop:expr;
        $(fn clone = $clone:expr;)*

        $(#[$owned_attr:meta])*
        pub struct $owned:ident;
        $(#[$borrowed_attr:meta])*
        pub struct $borrowed:ident;
    )
        => {
            ::foreign_types::foreign_type! {
                $(#[$impl_attr])*
                type CType = $ctype;
                fn drop = $drop;
                $(fn clone = $clone;)*
                $(#[$owned_attr])*
                pub struct $owned;
                $(#[$borrowed_attr])*
                pub struct $borrowed;
            }

            unsafe impl Send for $owned{}
            unsafe impl Send for $borrowed{}
            unsafe impl Sync for $owned{}
            unsafe impl Sync for $borrowed{}
        };
}

macro_rules! generic_foreign_type_and_impl_send_sync {
    (
        $(#[$impl_attr:meta])*
        type CType = $ctype:ty;
        fn drop = $drop:expr;
        $(fn clone = $clone:expr;)*

        $(#[$owned_attr:meta])*
        pub struct $owned:ident<T>;
        $(#[$borrowed_attr:meta])*
        pub struct $borrowed:ident<T>;
    ) => {
        $(#[$owned_attr])*
        pub struct $owned<T>(*mut $ctype, ::std::marker::PhantomData<T>);

        $(#[$impl_attr])*
        impl<T> ::foreign_types::ForeignType for $owned<T> {
            type CType = $ctype;
            type Ref = $borrowed<T>;

            #[inline]
            unsafe fn from_ptr(ptr: *mut $ctype) -> $owned<T> {
                $owned(ptr, ::std::marker::PhantomData)
            }

            #[inline]
            fn as_ptr(&self) -> *mut $ctype {
                self.0
            }
        }

        impl<T> Drop for $owned<T> {
            #[inline]
            fn drop(&mut self) {
                unsafe { $drop(self.0) }
            }
        }

        $(
            impl<T> Clone for $owned<T> {
                #[inline]
                fn clone(&self) -> $owned<T> {
                    unsafe {
                        let handle: *mut $ctype = $clone(self.0);
                        ::foreign_types::ForeignType::from_ptr(handle)
                    }
                }
            }

            impl<T> ::std::borrow::ToOwned for $borrowed<T> {
                type Owned = $owned<T>;
                #[inline]
                fn to_owned(&self) -> $owned<T> {
                    unsafe {
                        let handle: *mut $ctype =
                            $clone(::foreign_types::ForeignTypeRef::as_ptr(self));
                        $crate::ForeignType::from_ptr(handle)
                    }
                }
            }
        )*

        impl<T> ::std::ops::Deref for $owned<T> {
            type Target = $borrowed<T>;

            #[inline]
            fn deref(&self) -> &$borrowed<T> {
                unsafe { ::foreign_types::ForeignTypeRef::from_ptr(self.0) }
            }
        }

        impl<T> ::std::ops::DerefMut for $owned<T> {
            #[inline]
            fn deref_mut(&mut self) -> &mut $borrowed<T> {
                unsafe { ::foreign_types::ForeignTypeRef::from_ptr_mut(self.0) }
            }
        }

        impl<T> ::std::borrow::Borrow<$borrowed<T>> for $owned<T> {
            #[inline]
            fn borrow(&self) -> &$borrowed<T> {
                &**self
            }
        }

        impl<T> ::std::convert::AsRef<$borrowed<T>> for $owned<T> {
            #[inline]
            fn as_ref(&self) -> &$borrowed<T> {
                &**self
            }
        }

        $(#[$borrowed_attr])*
        pub struct $borrowed<T>(::foreign_types::Opaque, ::std::marker::PhantomData<T>);

        $(#[$impl_attr])*
        impl<T> ::foreign_types::ForeignTypeRef for $borrowed<T> {
            type CType = $ctype;
        }

        unsafe impl<T> Send for $owned<T>{}
        unsafe impl<T> Send for $borrowed<T>{}
        unsafe impl<T> Sync for $owned<T>{}
        unsafe impl<T> Sync for $borrowed<T>{}
    };
}
