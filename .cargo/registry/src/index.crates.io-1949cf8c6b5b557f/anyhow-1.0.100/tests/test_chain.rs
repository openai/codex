use anyhow::{anyhow, Chain, Error};

fn error() -> Error {
    anyhow!({ 0 }).context(1).context(2).context(3)
}

#[test]
fn test_iter() {
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
    let e = error();
    let mut chain = e.chain();
    assert_eq!(4, chain.len());
    assert_eq!((4, Some(4)), chain.size_hint());
    assert_eq!("3", chain.next().unwrap().to_string());
    assert_eq!(3, chain.len());
    assert_eq!((3, Some(3)), chain.size_hint());
    assert_eq!("0", chain.next_back().unwrap().to_string());
    assert_eq!(2, chain.len());
    assert_eq!((2, Some(2)), chain.size_hint());
    assert_eq!("2", chain.next().unwrap().to_string());
    assert_eq!(1, chain.len());
    assert_eq!((1, Some(1)), chain.size_hint());
    assert_eq!("1", chain.next_back().unwrap().to_string());
    assert_eq!(0, chain.len());
    assert_eq!((0, Some(0)), chain.size_hint());
    assert!(chain.next().is_none());
}

#[test]
fn test_default() {
    let mut c = Chain::default();
    assert!(c.next().is_none());
}

#[test]
#[allow(clippy::redundant_clone)]
fn test_clone() {
    let e = error();
    let mut chain = e.chain().clone();
    assert_eq!("3", chain.next().unwrap().to_string());
    assert_eq!("2", chain.next().unwrap().to_string());
    assert_eq!("1", chain.next().unwrap().to_string());
    assert_eq!("0", chain.next().unwrap().to_string());
    assert!(chain.next().is_none());
    assert!(chain.next_back().is_none());
}
