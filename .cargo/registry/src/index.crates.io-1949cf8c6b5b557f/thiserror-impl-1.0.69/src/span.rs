use proc_macro2::Span;
use syn::Member;

pub trait MemberSpan {
    fn member_span(&self) -> Span;
}

impl MemberSpan for Member {
    fn member_span(&self) -> Span {
        match self {
            Member::Named(ident) => ident.span(),
            Member::Unnamed(index) => index.span,
        }
    }
}
