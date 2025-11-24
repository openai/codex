#[cfg(not(any(
    feature = "once_cell_cache",
    feature = "lazy_static_cache",
    feature = "unsafe_cache"
)))]
macro_rules! get_cwd {
    () => {
        std::env::current_dir()?
    };
}

#[cfg(any(feature = "once_cell_cache", feature = "lazy_static_cache"))]
macro_rules! get_cwd {
    () => {
        $crate::CWD.as_path()
    };
}

#[cfg(feature = "unsafe_cache")]
macro_rules! get_cwd {
    () => {
        unsafe { $crate::CWD.as_path() }
    };
}
