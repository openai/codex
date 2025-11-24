#[test]
fn iproduct_hygiene() {
    let _ = itertools::iproduct!();
    let _ = itertools::iproduct!(0..6);
    let _ = itertools::iproduct!(0..6, 0..9);
    let _ = itertools::iproduct!(0..6, 0..9, 0..12);
}

#[test]
fn izip_hygiene() {
    let _ = itertools::izip!(0..6);
    let _ = itertools::izip!(0..6, 0..9);
    let _ = itertools::izip!(0..6, 0..9, 0..12);
}
