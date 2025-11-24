#[test]
fn test_create_map() {
    let _m = indexmap::indexmap! {
        1 => 2,
        7 => 1,
        2 => 2,
        3 => 3,
    };
}

#[test]
fn test_create_set() {
    let _s = indexmap::indexset! {
        1,
        7,
        2,
        3,
    };
}
