use crate::{NibbleVec, Nibblet};

fn v8_7_6_5() -> Nibblet {
    NibbleVec::from_byte_vec(vec![8 << 4 | 7, 6 << 4 | 5])
}

fn v11_10_9() -> Nibblet {
    let mut result = Nibblet::from_byte_vec(vec![11 << 4 | 10]);
    result.push(9);
    result
}

#[test]
fn get() {
    let nv = Nibblet::from_byte_vec(vec![3 << 4 | 7]);
    assert_eq!(nv.get(0), 3u8);
    assert_eq!(nv.get(1), 7u8);
}

#[test]
fn push() {
    let mut nv = Nibblet::new();
    let data = vec![0, 1, 3, 5, 7, 9, 11, 15];
    for val in data.iter() {
        nv.push(*val);
    }

    for (i, val) in data.iter().enumerate() {
        assert_eq!(nv.get(i), *val);
    }
}

fn split_test(nibble_vec: &Nibblet, idx: usize, first: Vec<u8>, second: Vec<u8>) {
    let mut init = nibble_vec.clone();
    let tail = init.split(idx);
    assert!(init == first[..]);
    assert!(tail == second[..]);
}

#[test]
fn split_even_length() {
    let even_length = v8_7_6_5();
    split_test(&even_length, 0, vec![], vec![8, 7, 6, 5]);
    split_test(&even_length, 1, vec![8], vec![7, 6, 5]);
    split_test(&even_length, 2, vec![8, 7], vec![6, 5]);
    split_test(&even_length, 4, vec![8, 7, 6, 5], vec![]);
}

#[test]
fn split_odd_length() {
    let odd_length = v11_10_9();
    split_test(&odd_length, 0, vec![], vec![11, 10, 9]);
    split_test(&odd_length, 1, vec![11], vec![10, 9]);
    split_test(&odd_length, 2, vec![11, 10], vec![9]);
    split_test(&odd_length, 3, vec![11, 10, 9], vec![]);
}

/// Join vec2 onto vec1 and ensure that the results matches the one expected.
fn join_test(vec1: &Nibblet, vec2: &Nibblet, result: Vec<u8>) {
    let joined = vec1.clone().join(vec2);
    assert!(joined == result[..]);
}

#[test]
fn join_even_length() {
    let v1 = v8_7_6_5();
    let v2 = v11_10_9();
    join_test(&v1, &v2, vec![8, 7, 6, 5, 11, 10, 9]);
    join_test(&v1, &v1, vec![8, 7, 6, 5, 8, 7, 6, 5]);
    join_test(&v1, &Nibblet::new(), vec![8, 7, 6, 5]);
    join_test(&Nibblet::new(), &v1, vec![8, 7, 6, 5]);
}

#[test]
fn join_odd_length() {
    let v1 = v8_7_6_5();
    let v2 = v11_10_9();
    join_test(&v2, &v1, vec![11, 10, 9, 8, 7, 6, 5]);
    join_test(&v2, &v2, vec![11, 10, 9, 11, 10, 9]);
    join_test(&v2, &Nibblet::new(), vec![11, 10, 9]);
}

#[test]
fn clone() {
    #[allow(clippy::redundant_clone)]
    let v1 = v8_7_6_5().clone();
    assert_eq!(v1.len(), 4);
}

/// Ensure that the last nibble is zeroed before reuse.
#[test]
fn memory_reuse() {
    let mut vec = Nibblet::new();
    vec.push(10);
    vec.push(1);
    // Pushing.
    vec.split(1);
    vec.push(2);
    assert_eq!(vec.get(1), 2);

    // Joining.
    vec.split(1);
    vec = vec.join(&Nibblet::from_byte_vec(vec![1 << 4 | 3, 5 << 4]));
    assert_eq!(vec.get(1), 1);
}

#[test]
fn from() {
    let v = vec![243, 2, 3, 251, 5, 6, 7, 8, 255];
    let n = Nibblet::from_byte_vec(v.clone());
    let n2 = Nibblet::from(&v[..]);
    assert_eq!(n, n2);
    let n3 = Nibblet::from(v);
    assert_eq!(n, n3);
}

#[test]
fn into() {
    let v = vec![243, 2, 3, 251, 5, 6, 7, 8, 255];
    {
        let n = Nibblet::from_byte_vec(v.clone());
        let v2: Vec<u8> = n.into();
        assert_eq!(v, v2);
    }
    {
        let n = Nibblet::from_byte_vec(v.clone());
        let v2: Vec<u8> = (&n).into();
        assert_eq!(v, v2);
    }
}

#[test]
fn as_bytes() {
    let v = vec![243, 2, 3, 251, 5, 6, 7, 8, 255];
    let n = Nibblet::from(&v[..]);
    assert_eq!(&v[..], n.as_bytes());
}

#[test]
fn into_bytes() {
    let v = vec![243, 2, 3, 251, 5, 6, 7, 8, 255];
    let n = Nibblet::from(&v[..]);
    assert_eq!(v, n.into_bytes());
}
