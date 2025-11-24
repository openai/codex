use futures::executor::block_on;
use futures::future::{ready, select_all};
use std::collections::HashSet;

#[test]
fn smoke() {
    let v = vec![ready(1), ready(2), ready(3)];

    let mut c = vec![1, 2, 3].into_iter().collect::<HashSet<_>>();

    let (i, idx, v) = block_on(select_all(v));
    assert!(c.remove(&i));
    assert_eq!(idx, 0);

    let (i, idx, v) = block_on(select_all(v));
    assert!(c.remove(&i));
    assert_eq!(idx, 0);

    let (i, idx, v) = block_on(select_all(v));
    assert!(c.remove(&i));
    assert_eq!(idx, 0);

    assert!(c.is_empty());
    assert!(v.is_empty());
}
