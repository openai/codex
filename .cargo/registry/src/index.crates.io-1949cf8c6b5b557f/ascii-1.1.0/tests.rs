extern crate ascii;

use ascii::{AsAsciiStr, AsciiChar, AsciiStr};
#[cfg(feature = "std")]
use ascii::{AsciiString, IntoAsciiString};

#[test]
#[cfg(feature = "std")]
fn ascii_vec() {
    let test = b"( ;";
    let a = AsciiStr::from_ascii(test).unwrap();
    assert_eq!(test.as_ascii_str(), Ok(a));
    assert_eq!("( ;".as_ascii_str(), Ok(a));
    let v = test.to_vec();
    assert_eq!(v.as_ascii_str(), Ok(a));
    assert_eq!("( ;".to_string().as_ascii_str(), Ok(a));
}

#[test]
fn to_ascii() {
    assert!("zoä华".as_ascii_str().is_err());
    assert!([127_u8, 128, 255].as_ascii_str().is_err());

    let arr = [AsciiChar::ParenOpen, AsciiChar::Space, AsciiChar::Semicolon];
    let a: &AsciiStr = (&arr[..]).into();
    assert_eq!(b"( ;".as_ascii_str(), Ok(a));
    assert_eq!("( ;".as_ascii_str(), Ok(a));
}

#[test]
#[cfg(feature = "std")]
fn into_ascii() {
    let arr = [AsciiChar::ParenOpen, AsciiChar::Space, AsciiChar::Semicolon];
    let v = AsciiString::from(arr.to_vec());
    assert_eq!(b"( ;".to_vec().into_ascii_string(), Ok(v.clone()));
    assert_eq!("( ;".to_string().into_ascii_string(), Ok(v.clone()));
    assert_eq!(b"( ;", AsRef::<[u8]>::as_ref(&v));

    let err = "zoä华".to_string().into_ascii_string().unwrap_err();
    assert_eq!(Err(err.ascii_error()), "zoä华".as_ascii_str());
    assert_eq!(err.into_source(), "zoä华");
    let err = vec![127, 128, 255].into_ascii_string().unwrap_err();
    assert_eq!(Err(err.ascii_error()), [127, 128, 255].as_ascii_str());
    assert_eq!(err.into_source(), &[127, 128, 255]);
}

#[test]
#[cfg(feature = "std")]
fn compare_ascii_string_ascii_str() {
    let v = b"abc";
    let ascii_string = AsciiString::from_ascii(&v[..]).unwrap();
    let ascii_str = AsciiStr::from_ascii(v).unwrap();
    assert!(ascii_string == ascii_str);
    assert!(ascii_str == ascii_string);
}

#[test]
#[cfg(feature = "std")]
fn compare_ascii_string_string() {
    let v = b"abc";
    let string = String::from_utf8(v.to_vec()).unwrap();
    let ascii_string = AsciiString::from_ascii(&v[..]).unwrap();
    assert!(string == ascii_string);
    assert!(ascii_string == string);
}

#[test]
#[cfg(feature = "std")]
fn compare_ascii_str_string() {
    let v = b"abc";
    let string = String::from_utf8(v.to_vec()).unwrap();
    let ascii_str = AsciiStr::from_ascii(&v[..]).unwrap();
    assert!(string == ascii_str);
    assert!(ascii_str == string);
}

#[test]
#[cfg(feature = "std")]
fn compare_ascii_string_str() {
    let v = b"abc";
    let sstr = ::std::str::from_utf8(v).unwrap();
    let ascii_string = AsciiString::from_ascii(&v[..]).unwrap();
    assert!(sstr == ascii_string);
    assert!(ascii_string == sstr);
}

#[test]
fn compare_ascii_str_str() {
    let v = b"abc";
    let sstr = ::std::str::from_utf8(v).unwrap();
    let ascii_str = AsciiStr::from_ascii(v).unwrap();
    assert!(sstr == ascii_str);
    assert!(ascii_str == sstr);
}

#[test]
#[allow(clippy::redundant_slicing)]
fn compare_ascii_str_slice() {
    let b = b"abc".as_ascii_str().unwrap();
    let c = b"ab".as_ascii_str().unwrap();
    assert_eq!(&b[..2], &c[..]);
    assert_eq!(c[1].as_char(), 'b');
}

#[test]
#[cfg(feature = "std")]
fn compare_ascii_string_slice() {
    let b = AsciiString::from_ascii("abc").unwrap();
    let c = AsciiString::from_ascii("ab").unwrap();
    assert_eq!(&b[..2], &c[..]);
    assert_eq!(c[1].as_char(), 'b');
}

#[test]
#[cfg(feature = "std")]
fn extend_from_iterator() {
    use std::borrow::Cow;

    let abc = "abc".as_ascii_str().unwrap();
    let mut s = abc.chars().collect::<AsciiString>();
    assert_eq!(s, abc);
    s.extend(abc);
    assert_eq!(s, "abcabc");

    let lines = "one\ntwo\nthree".as_ascii_str().unwrap().lines();
    s.extend(lines);
    assert_eq!(s, "abcabconetwothree");

    let cows = "ASCII Ascii ascii"
        .as_ascii_str()
        .unwrap()
        .split(AsciiChar::Space)
        .map(|case| {
            if case.chars().all(AsciiChar::is_uppercase) {
                Cow::from(case)
            } else {
                Cow::from(case.to_ascii_uppercase())
            }
        });
    s.extend(cows);
    s.extend(&[AsciiChar::LineFeed]);
    assert_eq!(s, "abcabconetwothreeASCIIASCIIASCII\n");
}
