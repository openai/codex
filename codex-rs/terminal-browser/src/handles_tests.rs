use pretty_assertions::assert_eq;

use super::BrowserHandles;

#[test]
fn handles_are_document_scoped_and_stale_after_refresh() {
    let mut handles = BrowserHandles::default();
    let first = handles.insert(/*backend_node_id*/ 42);
    assert_eq!(handles.resolve(&first).expect("resolve handle"), 42);

    handles.begin_snapshot();
    let second = handles.insert(/*backend_node_id*/ 42);

    assert_ne!(first, second);
    assert!(handles.resolve(&first).is_err());
    assert_eq!(handles.resolve(&second).expect("resolve new handle"), 42);
}
