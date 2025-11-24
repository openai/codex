macro_rules! escape {
    ($($inner:expr),*) => {
        concat!("\x1B[", $($inner),*, "m")
    };
}
