#[test]
fn issue1108() {
    let data = "impl<x<>>::x for";
    let _ = syn::parse_file(data);
}
