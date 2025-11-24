//! In this example we build an [S-expression](https://en.wikipedia.org/wiki/S-expression)
//! parser and tiny [lisp](https://en.wikipedia.org/wiki/Lisp_(programming_language)) interpreter.
//! Lisp is a simple type of language made up of Atoms and Lists, forming easily parsable trees.

#![cfg(feature = "alloc")]

mod parser;

fn main() {
    let expression_1 = "((if (= (+ 3 (/ 9 3))
         (* 2 3))
     *
     /)
  456 123)";
    println!(
        "\"{}\"\nevaled gives us: {:?}",
        expression_1,
        parser::eval_from_str(expression_1)
    );
}
