#![cfg(test)]

use crate::grammar::free_variables::FreeVariables;
use crate::test_util::{expect_debug, normalized_grammar};
use crate::tls::Tls;

#[test]
fn other_names() {
    // Check that `Foo` does not end up in the list of free variables.
    let _tls = Tls::test();
    let grammar = normalized_grammar(
        r#"
grammar<'a, T>(x: &'a mut Foo, y: Vec<T>);

pub Foo: () = ();
"#,
    );

    let p0 = &grammar.parameters[0];
    expect_debug(
        p0.ty.free_variables(&grammar.type_parameters),
        "[
    Lifetime(
        'a
    )
]",
    );

    let p1 = &grammar.parameters[1];
    expect_debug(
        p1.ty.free_variables(&grammar.type_parameters),
        "[
    Id(
        Atom('T' type=inline)
    )
]",
    );
}
