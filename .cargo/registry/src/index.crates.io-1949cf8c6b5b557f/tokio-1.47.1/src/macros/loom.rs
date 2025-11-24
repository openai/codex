macro_rules! if_loom {
    ($($t:tt)*) => {{
        #[cfg(loom)]
        {
            $($t)*
        }
    }}
}
