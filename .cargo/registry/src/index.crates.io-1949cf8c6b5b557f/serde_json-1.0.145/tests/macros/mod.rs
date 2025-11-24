#![allow(unused_macro_rules)]

macro_rules! json_str {
    ([]) => {
        "[]"
    };
    ([ $e0:tt $(, $e:tt)* $(,)? ]) => {
        concat!("[",
            json_str!($e0),
            $(",", json_str!($e),)*
        "]")
    };
    ({}) => {
        "{}"
    };
    ({ $k0:tt : $v0:tt $(, $k:tt : $v:tt)* $(,)? }) => {
        concat!("{",
            stringify!($k0), ":", json_str!($v0),
            $(",", stringify!($k), ":", json_str!($v),)*
        "}")
    };
    (($other:tt)) => {
        $other
    };
    ($other:tt) => {
        stringify!($other)
    };
}

macro_rules! pretty_str {
    ($json:tt) => {
        pretty_str_impl!("", $json)
    };
}

macro_rules! pretty_str_impl {
    ($indent:expr, []) => {
        "[]"
    };
    ($indent:expr, [ $e0:tt $(, $e:tt)* $(,)? ]) => {
        concat!("[\n  ",
            $indent, pretty_str_impl!(concat!("  ", $indent), $e0),
            $(",\n  ", $indent, pretty_str_impl!(concat!("  ", $indent), $e),)*
        "\n", $indent, "]")
    };
    ($indent:expr, {}) => {
        "{}"
    };
    ($indent:expr, { $k0:tt : $v0:tt $(, $k:tt : $v:tt)* $(,)? }) => {
        concat!("{\n  ",
            $indent, stringify!($k0), ": ", pretty_str_impl!(concat!("  ", $indent), $v0),
            $(",\n  ", $indent, stringify!($k), ": ", pretty_str_impl!(concat!("  ", $indent), $v),)*
        "\n", $indent, "}")
    };
    ($indent:expr, ($other:tt)) => {
        $other
    };
    ($indent:expr, $other:tt) => {
        stringify!($other)
    };
}
