mod common;

use self::common::maybe_install_handler;
use eyre::{eyre, Report};

fn error() -> Report {
    eyre!({ 0 }).wrap_err(1).wrap_err(2).wrap_err(3)
}

#[test]
fn test_iter() {
    maybe_install_handler().unwrap();

    let e = error();
    let mut chain = e.chain();
    assert_eq!("3", chain.next().unwrap().to_string());
    assert_eq!("2", chain.next().unwrap().to_string());
    assert_eq!("1", chain.next().unwrap().to_string());
    assert_eq!("0", chain.next().unwrap().to_string());
    assert!(chain.next().is_none());
    assert!(chain.next_back().is_none());
}

#[test]
fn test_rev() {
    maybe_install_handler().unwrap();

    let e = error();
    let mut chain = e.chain().rev();
    assert_eq!("0", chain.next().unwrap().to_string());
    assert_eq!("1", chain.next().unwrap().to_string());
    assert_eq!("2", chain.next().unwrap().to_string());
    assert_eq!("3", chain.next().unwrap().to_string());
    assert!(chain.next().is_none());
    assert!(chain.next_back().is_none());
}

#[test]
fn test_len() {
    maybe_install_handler().unwrap();

    let e = error();
    let mut chain = e.chain();
    assert_eq!(4, chain.len());
    assert_eq!("3", chain.next().unwrap().to_string());
    assert_eq!(3, chain.len());
    assert_eq!("0", chain.next_back().unwrap().to_string());
    assert_eq!(2, chain.len());
    assert_eq!("2", chain.next().unwrap().to_string());
    assert_eq!(1, chain.len());
    assert_eq!("1", chain.next_back().unwrap().to_string());
    assert_eq!(0, chain.len());
    assert!(chain.next().is_none());
}
