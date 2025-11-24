macro_rules! should_parse {
    ($name:ident, { $($in:tt)* }) => {
        #[test]
        fn $name() {
            // Make sure we can parse the file!
            syn::parse_file(stringify!($($in)*)).unwrap();
        }
    }
}

should_parse!(generic_associated_type, {
    impl Foo {
        type Item = &'a i32;
        fn foo<'a>(&'a self) -> Self::Item<'a> {}
    }
});

#[rustfmt::skip]
should_parse!(const_generics_use, {
    type X = Foo<5>;
    type Y = Foo<"foo">;
    type Z = Foo<X>;
    type W = Foo<{ X + 10 }>;
});

should_parse!(trailing_plus_type, {
    type A = Box<Foo>;
    type A = Box<Foo + 'a>;
    type A = Box<'a + Foo>;
});

should_parse!(generic_associated_type_where, {
    trait Foo {
        type Item;
        fn foo<T>(&self, t: T) -> Self::Item<T>;
    }
});

should_parse!(match_with_block_expr, {
    fn main() {
        match false {
            _ => {}.a(),
        }
    }
});
