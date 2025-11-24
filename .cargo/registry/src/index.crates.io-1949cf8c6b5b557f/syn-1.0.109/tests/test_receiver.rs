use syn::{parse_quote, FnArg, Receiver, TraitItemMethod};

#[test]
fn test_by_value() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn by_value(self: Self);
    };
    match sig.receiver() {
        Some(FnArg::Typed(_)) => (),
        value => panic!("expected FnArg::Typed, got {:?}", value),
    }
}

#[test]
fn test_by_mut_value() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn by_mut(mut self: Self);
    };
    match sig.receiver() {
        Some(FnArg::Typed(_)) => (),
        value => panic!("expected FnArg::Typed, got {:?}", value),
    }
}

#[test]
fn test_by_ref() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn by_ref(self: &Self);
    };
    match sig.receiver() {
        Some(FnArg::Typed(_)) => (),
        value => panic!("expected FnArg::Typed, got {:?}", value),
    }
}

#[test]
fn test_by_box() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn by_box(self: Box<Self>);
    };
    match sig.receiver() {
        Some(FnArg::Typed(_)) => (),
        value => panic!("expected FnArg::Typed, got {:?}", value),
    }
}

#[test]
fn test_by_pin() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn by_pin(self: Pin<Self>);
    };
    match sig.receiver() {
        Some(FnArg::Typed(_)) => (),
        value => panic!("expected FnArg::Typed, got {:?}", value),
    }
}

#[test]
fn test_explicit_type() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn explicit_type(self: Pin<MyType>);
    };
    match sig.receiver() {
        Some(FnArg::Typed(_)) => (),
        value => panic!("expected FnArg::Typed, got {:?}", value),
    }
}

#[test]
fn test_value_shorthand() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn value_shorthand(self);
    };
    match sig.receiver() {
        Some(FnArg::Receiver(Receiver {
            reference: None,
            mutability: None,
            ..
        })) => (),
        value => panic!("expected FnArg::Receiver without ref/mut, got {:?}", value),
    }
}

#[test]
fn test_mut_value_shorthand() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn mut_value_shorthand(mut self);
    };
    match sig.receiver() {
        Some(FnArg::Receiver(Receiver {
            reference: None,
            mutability: Some(_),
            ..
        })) => (),
        value => panic!("expected FnArg::Receiver with mut, got {:?}", value),
    }
}

#[test]
fn test_ref_shorthand() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn ref_shorthand(&self);
    };
    match sig.receiver() {
        Some(FnArg::Receiver(Receiver {
            reference: Some(_),
            mutability: None,
            ..
        })) => (),
        value => panic!("expected FnArg::Receiver with ref, got {:?}", value),
    }
}

#[test]
fn test_ref_mut_shorthand() {
    let TraitItemMethod { sig, .. } = parse_quote! {
        fn ref_mut_shorthand(&mut self);
    };
    match sig.receiver() {
        Some(FnArg::Receiver(Receiver {
            reference: Some(_),
            mutability: Some(_),
            ..
        })) => (),
        value => panic!("expected FnArg::Receiver with ref+mut, got {:?}", value),
    }
}
