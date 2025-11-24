#[test]
fn issue1108() {
    let data = "impl<x<>>::x for";
    _ = syn::parse_file(data);
}
