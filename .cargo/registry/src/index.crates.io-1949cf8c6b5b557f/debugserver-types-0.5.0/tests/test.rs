extern crate debugserver_types;

#[test]
fn sanity() {
    // Check that we generated someting at least
    fn test(_: &debugserver_types::ProtocolMessage, _: &debugserver_types::VariablesResponseBody) {}
    let _ = test;
}
