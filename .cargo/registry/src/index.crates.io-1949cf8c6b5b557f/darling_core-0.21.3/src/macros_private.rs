macro_rules! path {
    ($($path:tt)+) => {
        ::syn::parse_quote!($($path)+)
        //stringify!($($path)+).parse().unwrap()
    };
}
