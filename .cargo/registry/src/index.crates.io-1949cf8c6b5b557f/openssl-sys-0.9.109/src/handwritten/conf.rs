use super::super::*;

const_ptr_api! {
    extern "C" {
        pub fn NCONF_new(meth: #[const_ptr_if(libressl400)] CONF_METHOD) -> *mut CONF;
    }
}

extern "C" {
    #[cfg(not(libressl400))]
    pub fn NCONF_default() -> *mut CONF_METHOD;
    pub fn NCONF_free(conf: *mut CONF);
}
