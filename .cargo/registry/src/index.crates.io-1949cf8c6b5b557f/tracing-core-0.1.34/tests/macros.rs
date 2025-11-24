use tracing_core::{
    callsite::Callsite,
    metadata,
    metadata::{Kind, Level, Metadata},
    subscriber::Interest,
};

#[test]
fn metadata_macro_api() {
    // This test should catch any inadvertent breaking changes
    // caused by changes to the macro.
    struct TestCallsite;

    impl Callsite for TestCallsite {
        fn set_interest(&self, _: Interest) {
            unimplemented!("test")
        }
        fn metadata(&self) -> &Metadata<'_> {
            unimplemented!("test")
        }
    }

    static CALLSITE: TestCallsite = TestCallsite;
    let _metadata = metadata! {
        name: "test_metadata",
        target: "test_target",
        level: Level::DEBUG,
        fields: &["foo", "bar", "baz"],
        callsite: &CALLSITE,
        kind: Kind::SPAN,
    };
    let _metadata = metadata! {
        name: "test_metadata",
        target: "test_target",
        level: Level::TRACE,
        fields: &[],
        callsite: &CALLSITE,
        kind: Kind::EVENT,
    };
    let _metadata = metadata! {
        name: "test_metadata",
        target: "test_target",
        level: Level::INFO,
        fields: &[],
        callsite: &CALLSITE,
        kind: Kind::EVENT
    };
}
