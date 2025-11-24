use ts_rs_macros::TS;

// https://github.com/Aleph-Alpha/ts-rs/issues/335
#[derive(TS)]
#[ts(export, export_to = "generics/flatten/")]
struct Item<D> {
    id: String,
    #[ts(flatten)]
    inner: D,
}

#[derive(TS)]
#[ts(export, export_to = "generics/flatten/")]
struct TwoParameters<A, B> {
    id: String,
    #[ts(flatten)]
    a: A,
    #[ts(flatten)]
    b: B,
    ab: (A, B),
}

#[derive(TS)]
#[ts(export, export_to = "generics/flatten/")]
enum Enum<A, B> {
    A {
        #[ts(flatten)]
        a: A,
    },
    B {
        #[ts(flatten)]
        b: B,
    },
    AB(A, B),
}

#[test]
fn flattened_generic_parameters() {
    use ts_rs::TS;

    #[derive(TS)]
    struct Inner {
        x: i32,
    }

    assert_eq!(Item::<()>::decl(), "type Item<D> = { id: string, } & D;");
    assert_eq!(
        TwoParameters::<(), ()>::decl(),
        "type TwoParameters<A, B> = { id: string, ab: [A, B], } & A & B;"
    );
    assert_eq!(
        Enum::<(), ()>::decl(),
        "type Enum<A, B> = { \"A\": A } | { \"B\": B } | { \"AB\": [A, B] };"
    );
}
