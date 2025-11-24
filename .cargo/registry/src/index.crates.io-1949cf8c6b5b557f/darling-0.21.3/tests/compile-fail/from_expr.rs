use darling::FromMeta;

/// This usage of `from_expr` is VALID because there are no unit variants that should conflict with the
/// implementation.
#[derive(FromMeta)]
#[darling(from_expr = |expr| Ok(HasNoUnits::Variant2 { other: format!("{:?}", expr) }))]
enum HasNoUnits {
    Variant1 { field: String },
    Variant2 { other: String },
}

/// This usage of `from_expr` is invalid because unit variants already generate a from_expr
/// method, and we don't allow using the from_expr override when it conflicts with the macro's
/// "normal" operation.
#[derive(FromMeta)]
#[darling(from_expr = |expr| Ok(HasUnits::Variant2))]
enum HasUnits {
    Variant1 { field: String },
    Variant2,
}

fn newtype_from_expr(_expr: &syn::Expr) -> darling::Result<Newtype> {
    Ok(Newtype(true))
}

// This usage of `from_expr` is invalid because newtype structs call the inner type's `from_meta`
// directly from their `from_meta`, so the custom `from_expr` will never be called in normal usage.
#[derive(FromMeta)]
#[darling(from_expr = newtype_from_expr)]
struct Newtype(bool);

fn main() {}
