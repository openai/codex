
#[macro_use] extern crate maplit;

#[test]
#[allow(unused_parens)]
fn test_parse() {
    let mut m = hashmap!{};
    m.insert(1, 1);
    hashmap!{1 => 1};
    hashmap!{1 => 1,};
    hashmap!{1 + 1 => 1, 2 + 1 => 2};
    hashmap!{1 + 1 => 1, 2 + 1 => 2,};
    hashmap!{{1 + 2} => 1, (1 + 3) => {0 + 2}};
    let m = hashmap!{"a".to_string() => 1 + 2, "b".to_string() => 1 + 3};
    assert_eq!(m["a"], 3);
    assert_eq!(m["b"], 4);
    let m = hashmap!{"a".to_string() => 1 + 2, "b".to_string() => 1 + 3, };
    assert_eq!(m["a"], 3);
    assert_eq!(m["b"], 4);

    let mut s = hashset!{};
    s.insert(1);
    hashset!{1};
    hashset!{1,};
    hashset!{1, 2};
    hashset!{1, 2,};
    hashset!{1 + 1, 2 + 1};
    hashset!{1 + 1, 2 + 1,};
    hashset!{{1 + 1}, (2 + 1)};
}

#[test]
fn hashset() {
    let mut set = hashset!{};
    assert!(set.is_empty());
    set.insert(2);
    let set = hashset!{1};
    assert_eq!(set.len(), 1);
    let set = hashset!{2, 3};
    assert_eq!(set.len(), 2);
    // Test that we can use many elements without hitting the macro recursion limit
    let set = hashset!{1,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
        2,3,4,5,6,7,8,9,0,1,2,3,4,5,6,
    };
    assert_eq!(set.len(), 10);
}
