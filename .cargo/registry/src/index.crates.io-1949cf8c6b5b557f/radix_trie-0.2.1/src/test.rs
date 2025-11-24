use crate::keys::TrieKey;
use crate::{Trie, TrieCommon};
use std::collections::HashSet;
use std::iter::FromIterator;

const TEST_DATA: [(&'static str, u32); 7] = [
    ("abcdefgh", 19),
    ("abcdef", 18),
    ("abcd", 17),
    ("ab", 16),
    ("a", 15),
    ("acbdef", 30),
    ("bcdefgh", 29),
];

fn test_trie() -> Trie<&'static str, u32> {
    let mut trie = Trie::new();

    for &(key, val) in &TEST_DATA {
        trie.insert(key, val);
        assert!(trie.check_integrity());
    }

    trie
}

#[test]
fn get_nonexistant() {
    let trie = test_trie();
    assert!(trie.get(&"nonexistant").is_none());
    assert!(trie.get(&"").is_none());
}

#[test]
fn subtrie_nonexistant() {
    let trie = test_trie();
    assert!(trie.subtrie(&"nonexistant").is_none());
    assert!(trie.subtrie(&"").is_some());
}

#[test]
fn unicode() {
    let mut trie = Trie::new();
    trie.insert("bär", 1);
    trie.insert("bären", 2);

    assert_eq!(*trie.get("bär").unwrap(), 1);
    let values = trie
        .get_raw_descendant("bä")
        .unwrap()
        .values()
        .collect::<HashSet<_>>();
    assert_eq!([1, 2].iter().collect::<HashSet<_>>(), values);
}

#[test]
fn subtrie() {
    let mut trie = Trie::new();
    trie.insert("hello", 55);
    assert!(trie.subtrie(&"h").is_none());
    assert!(trie.subtrie(&"hello").is_some());
}
#[test]
fn subtrie_string() {
    let mut trie = Trie::new();
    trie.insert("hello".to_string(), 55);

    let h = "h".to_string();
    let hello = "hello".to_string();

    assert!(trie.subtrie(&h).is_none());
    assert!(trie.subtrie(&hello).is_some());
}

#[test]
fn empty_key() {
    let mut trie = test_trie();
    trie.insert(&"", 99);
    assert_eq!(*trie.get(&"").unwrap(), 99);
    assert_eq!(trie.remove(&""), Some(99));
}

#[test]
fn insert() {
    let trie = test_trie();

    for &(key, val) in &TEST_DATA {
        assert_eq!(*trie.get(&key).unwrap(), val);
    }

    assert!(trie.check_integrity());
    assert_eq!(trie.len(), TEST_DATA.len());
}

#[test]
fn insert_replace() {
    let mut trie = Trie::new();
    assert_eq!(trie.insert("haskell", 18), None);
    let length = trie.len();
    assert_eq!(trie.insert("haskell", 36), Some(18));
    assert_eq!(trie.len(), length);
}

#[test]
fn map_with_default() {
    let mut trie = test_trie();
    trie.map_with_default(&"abcd", |x| *x = *x + 1, 42);
    assert_eq!(*trie.get(&"abcd").unwrap(), 17 + 1);
    trie.map_with_default(&"zzz", |x| *x = *x + 1, 42);
    assert_eq!(*trie.get(&"zzz").unwrap(), 42);
}

#[test]
fn remove() {
    let mut trie = test_trie();

    // Remove.
    for &(key, val) in &TEST_DATA {
        let res = trie.remove(&key);
        assert_eq!(res, Some(val));
        assert!(trie.check_integrity());
    }

    // Check non-existance.
    for &(key, _) in &TEST_DATA {
        assert!(trie.get(&key).is_none());
    }
}

#[test]
fn remove_simple() {
    let mut trie = Trie::new();

    trie.insert("HELL", 66);
    trie.insert("HELLO", 77);
    let val = trie.remove(&"HELLO");
    assert_eq!(val, Some(77));
}

#[test]
fn remove_non_existent() {
    let mut trie = Trie::new();

    trie.insert("acab", true);

    assert_eq!(trie.remove(&"abc"), None);
    assert_eq!(trie.remove(&"acaba"), None);
    assert_eq!(trie.remove(&"a"), None);
    assert_eq!(trie.remove(&""), None);
    assert_eq!(trie.len(), 1);

    trie.insert("acaz", true);

    assert_eq!(trie.remove(&"acb"), None);
    assert_eq!(trie.remove(&"acaca"), None);
    assert_eq!(trie.remove(&"aca"), None);
    assert_eq!(trie.len(), 2);
}

#[test]
fn nearest_ancestor_root() {
    let mut trie = Trie::new();
    trie.insert("", 55);
    assert_eq!(trie.get_ancestor_value(&""), Some(&55));
}

#[test]
fn nearest_ancestor() {
    let trie = test_trie();
    assert_eq!(trie.get_ancestor_value(&""), None);

    // Test identity prefixes.
    for &(key, val) in &TEST_DATA {
        assert_eq!(trie.get_ancestor_value(&key), Some(&val));
    }

    assert_eq!(trie.get_ancestor_value(&"abcdefg"), trie.get(&"abcdef"));
    assert_eq!(trie.get_ancestor_value(&"abcde"), trie.get(&"abcd"));
    assert_eq!(trie.get_ancestor_value(&"aauksdjk"), trie.get(&"a"));
}

#[test]
fn nearest_ancestor_no_child_fn() {
    let mut t = Trie::new();
    t.insert("ab", 5);
    let anc = t.get_ancestor(&"abc").unwrap();
    assert_eq!(*anc.value().unwrap(), 5);
}

#[test]
fn raw_ancestor() {
    let mut t = Trie::new();

    for &(key, _) in &TEST_DATA {
        assert_eq!(t.get_raw_ancestor(&key).key(), t.key());
    }

    t.insert("wow", 0);
    t.insert("hella", 1);
    t.insert("hellb", 2);

    // Ancestor should be "hell" node.
    let anc = t.get_raw_ancestor(&"hello");
    assert_eq!(anc.len(), 2);
}

// Check that the subtrie prefix is correct for raw_ancestor.
#[test]
fn raw_ancestor_prefix() {
    let mut t = Trie::new();

    t.insert("abac", ());
    t.insert("abaz", ());

    let anc = t.get_raw_ancestor(&"aba");
    assert_eq!(anc.prefix, "aba".encode());
}

#[test]
fn iter() {
    type Set = HashSet<(&'static str, u32)>;
    let trie = test_trie();
    let expected = TEST_DATA.iter().map(|&x| x).collect::<Set>();
    let observed = trie.iter().map(|(&k, &v)| (k, v)).collect::<Set>();
    assert_eq!(expected, observed);
}

#[test]
fn get_raw_descendant() {
    let trie = test_trie();
    assert_eq!(
        trie.get_raw_descendant(&"abcdefgh").and_then(|t| t.value()),
        Some(&19)
    );
    assert_eq!(
        trie.get_raw_descendant(&"abcdefg").and_then(|t| t.value()),
        Some(&19)
    );
    assert!(trie.get_raw_descendant(&"acbg").is_none());
}

#[test]
fn raw_descendant_prefix() {
    let mut t = Trie::new();

    t.insert("abczzzz", ());
    t.insert("abcaaaa", ());

    assert_eq!(t.get_raw_descendant(&"a").unwrap().prefix, "abc".encode());
    assert_eq!(t.get_raw_descendant(&"abc").unwrap().prefix, "abc".encode());
    assert_eq!(
        t.get_raw_descendant(&"abca").unwrap().prefix,
        "abcaaaa".encode()
    );
}

#[test]
fn get_prefix_bug() {
    let mut trie = Trie::new();
    trie.insert("abdc", 5);
    trie.insert("abde", 6);
    assert!(trie.get(&"abc").is_none());
}

#[test]
fn get_ancestor_bug() {
    let mut trie = Trie::new();
    trie.insert("abc", 1);
    trie.insert("abcde", 2);
    assert_eq!(trie.get_ancestor_value(&"abcdz"), Some(&1));
}

#[test]
fn root_replace_bug() {
    let mut trie = Trie::new();
    trie.insert("a", ());
    trie.insert("p", ());
    trie.remove(&"a");
    assert_eq!(trie.len(), 1);
    trie.remove(&"p");
    assert_eq!(trie.len(), 0);
}

#[test]
fn subtrie_insert() {
    let mut trie = Trie::new();
    trie.insert("abc", 3);
    {
        let mut subtrie = trie.subtrie_mut(&"abc").unwrap();
        assert_eq!(subtrie.insert("somerandomshit", 666), Err(()));
        assert_eq!(subtrie.insert("abcdef", 6), Ok(None));
        assert_eq!(subtrie.insert("abc", 9), Ok(Some(3)));
    }
    assert_eq!(trie.get(&"abc"), Some(&9));
    assert_eq!(trie.get(&"abcdef"), Some(&6));
    assert_eq!(trie.len(), 2);
}

#[test]
fn subtrie_len() {
    let trie = test_trie();
    assert_eq!(trie.subtrie(&"ab").unwrap().len(), 4);
    assert_eq!(trie.subtrie(&"a").unwrap().len(), 6);
    assert_eq!(trie.subtrie(&"").unwrap().len(), trie.len());
    assert_eq!(trie.subtrie(&"bcdefgh").unwrap().len(), 1);
}

// Subtrie functions that return references should return references valid for
// the lifetime of the *original trie* from which they were borrowed, NOT
// just the lifetime of the subtrie (which may be shorter).
#[test]
fn subtrie_lifetime() {
    let trie = test_trie();
    let subtrie_value = {
        let subtrie = trie.subtrie(&"ab").unwrap();
        subtrie.value()
    };
    assert_eq!(*subtrie_value.unwrap(), 16);
}

#[test]
fn subtrie_mut_lifetime() {
    let mut trie = test_trie();
    let subtrie_value = {
        let mut subtrie = trie.subtrie_mut(&"ab").unwrap();
        *subtrie.value_mut().unwrap() = 999;
        subtrie.value()
    };
    assert_eq!(*subtrie_value.unwrap(), 999);
}

#[test]
fn ancestor_key() {
    let trie = test_trie();
    let subtrie = trie.get_ancestor(&"abcde").unwrap();
    assert_eq!(*subtrie.key().unwrap(), "abcd");
    assert_eq!(subtrie.prefix, "abcd".encode());
    assert_eq!(*subtrie.get(&"abcdef").unwrap().unwrap(), 18);
    assert_eq!(*subtrie.get(&"abcdefgh").unwrap().unwrap(), 19);
}

#[test]
fn child_subtrie_keys() {
    let trie = test_trie();
    let subtrie = trie.subtrie(&"abcd").unwrap();
    for subsubtrie in subtrie.children() {
        // This subtrie should be for "abcde".
        assert_eq!(*subsubtrie.get(&"abcdef").unwrap().unwrap(), 18);
        assert_eq!(*subsubtrie.get(&"abcdefgh").unwrap().unwrap(), 19);
    }
}

#[test]
fn int_keys() {
    let mut trie = Trie::new();
    trie.insert(0x00ffu64, "asdf");
    trie.insert(0xdeadbeefu64, "asdf");
    assert!(trie.check_integrity());
}

#[test]
fn from_iter() {
    let trie: Trie<&str, u32> = Trie::from_iter(vec![("test", 10), ("hello", 12)]);
    assert_eq!(*trie.get(&"test").unwrap(), 10);
    assert_eq!(*trie.get(&"hello").unwrap(), 12);
    assert_eq!(trie.len(), 2);
}

#[test]
fn test_get_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "dir");
    assert_eq!(*trie.get("/boot").unwrap(), "dir");
}

#[test]
fn test_get_mut_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "dir");
    assert_eq!(*trie.get_mut("/boot").unwrap(), "dir");
}

#[test]
fn test_remove_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "dir");
    assert_eq!(trie.remove("/boot").unwrap(), "dir");
}

#[test]
fn test_subtrie_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "dir");
    trie.insert("/boot/lol".to_string(), "dir");
    trie.insert("/bleh".to_string(), "ohi");
    let subtrie = trie.subtrie("/boot").unwrap();
    assert_eq!(*subtrie.value().unwrap(), "dir");
}

#[test]
fn test_subtrie_mut_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "dir");
    trie.insert("/boot/lol".to_string(), "dir");
    trie.insert("/bleh".to_string(), "ohi");
    let subtrie = trie.subtrie_mut("/boot").unwrap();
    assert_eq!(*subtrie.value().unwrap(), "dir");
}

#[test]
fn test_get_ancestor_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "ancestor");
    trie.insert("/boot/lol".to_string(), "dir");
    trie.insert("/bleh".to_string(), "ohi");
    let subtrie = trie.get_ancestor("/boot/lo").unwrap();
    assert_eq!(*subtrie.value().unwrap(), "ancestor");
}

#[test]
fn test_get_ancestor_value_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "ancestor");
    trie.insert("/boot/lol".to_string(), "dir");
    trie.insert("/bleh".to_string(), "ohi");
    let ancestor_val = trie.get_ancestor_value("/boot/lo").unwrap();
    assert_eq!(*ancestor_val, "ancestor");
}

#[test]
fn test_get_raw_ancestor_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "boot");
    trie.insert("/boot/lol".to_string(), "dir");
    trie.insert("/bleh".to_string(), "ohi");
    let subtrie = trie.get_raw_ancestor("/boot/lo");
    assert_eq!(*subtrie.value().unwrap(), "boot");
}

#[test]
fn test_get_raw_descendant_borrow() {
    let mut trie = Trie::new();
    trie.insert("/boot".to_string(), "dir");
    trie.insert("/boot/lol".to_string(), "lol");
    trie.insert("/bleh".to_string(), "ohi");
    let subtrie = trie.get_raw_descendant("/boot/lo").unwrap();
    assert_eq!(*subtrie.value().unwrap(), "lol");
}

#[test]
fn test_prefix() {
    let mut t = Trie::<u8, ()>::new();
    t.insert(0xf1, ());
    t.remove(&0xf2);
    t.insert(0xf2, ());
    println!("{:#?}", t);
    assert_eq!(t.prefix(), [].as_ref());
    let first = t.children().next().unwrap();
    assert_eq!(first.prefix(), [0xf].as_ref());
    let mut c = first.children();
    let second = c.next().unwrap();
    let third = c.next().unwrap();
    assert!(c.next().is_none());
    assert_eq!(second.prefix(), [0x1].as_ref());
    assert_eq!(third.prefix(), [0x2].as_ref());
}

#[test]
fn clone() {
    let mut t1 = test_trie();
    let mut t2 = t1.clone();

    assert_eq!(t1, t2);
    t1.insert("abc", 22);

    assert_ne!(t1, t2);
    t2.insert("abc", 22);

    assert_eq!(t1, t2);
}
