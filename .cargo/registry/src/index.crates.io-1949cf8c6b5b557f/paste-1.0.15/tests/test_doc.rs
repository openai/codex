#![allow(clippy::let_underscore_untyped)]

use paste::paste;

#[test]
fn test_paste_doc() {
    macro_rules! m {
        ($ret:ident) => {
            paste! {
                #[doc = "Create a new [`" $ret "`] object."]
                fn new() -> $ret { todo!() }
            }
        };
    }

    struct Paste;
    m!(Paste);

    let _ = new;
}

macro_rules! get_doc {
    (#[doc = $literal:tt]) => {
        $literal
    };
}

#[test]
fn test_escaping() {
    let doc = paste! {
        get_doc!(#[doc = "s\"" r#"r#""#])
    };

    let expected = "s\"r#\"";
    assert_eq!(doc, expected);
}

#[test]
fn test_literals() {
    let doc = paste! {
        get_doc!(#[doc = "int=" 0x1 " bool=" true " float=" 0.01])
    };

    let expected = "int=0x1 bool=true float=0.01";
    assert_eq!(doc, expected);
}

#[test]
fn test_case() {
    let doc = paste! {
        get_doc!(#[doc = "HTTP " get:upper "!"])
    };

    let expected = "HTTP GET!";
    assert_eq!(doc, expected);
}

// https://github.com/dtolnay/paste/issues/63
#[test]
fn test_stringify() {
    macro_rules! create {
        ($doc:expr) => {
            paste! {
                #[doc = $doc]
                pub struct Struct;
            }
        };
    }

    macro_rules! forward {
        ($name:ident) => {
            create!(stringify!($name));
        };
    }

    forward!(documentation);

    let _ = Struct;
}
