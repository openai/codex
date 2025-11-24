//! In this example we build an [S-expression](https://en.wikipedia.org/wiki/S-expression)
//! parser and tiny [lisp](https://en.wikipedia.org/wiki/Lisp_(programming_language)) interpreter.
//! Lisp is a simple type of language made up of Atoms and Lists, forming easily parsable trees.

use winnow::{
    ascii::{alpha1, digit1, multispace0, multispace1},
    combinator::alt,
    combinator::repeat,
    combinator::{cut_err, opt},
    combinator::{delimited, preceded, terminated},
    error::ContextError,
    error::StrContext,
    prelude::*,
    token::one_of,
};

/// We start with a top-level function to tie everything together, letting
/// us call eval on a string directly
pub(crate) fn eval_from_str(src: &str) -> Result<Expr, String> {
    parse_expr
        .parse(src)
        .map_err(|e| e.to_string())
        .and_then(|exp| eval_expression(exp).ok_or_else(|| "Eval failed".to_owned()))
}

/// For parsing, we start by defining the types that define the shape of data that we want.
/// In this case, we want something tree-like
///
/// The remaining half is Lists. We implement these as recursive Expressions.
/// For a list of numbers, we have `'(1 2 3)`, which we'll parse to:
/// ```
/// Expr::Quote(vec![Expr::Constant(Atom::Num(1)),
///                  Expr::Constant(Atom::Num(2)),
///                  Expr::Constant(Atom::Num(3))])
/// Quote takes an S-expression and prevents evaluation of it, making it a data
/// structure that we can deal with programmatically. Thus any valid expression
/// is also a valid data structure in Lisp itself.
#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum Expr {
    Constant(Atom),
    /// (func-name arg1 arg2)
    Application(Box<Expr>, Vec<Expr>),
    /// (if predicate do-this)
    If(Box<Expr>, Box<Expr>),
    /// (if predicate do-this otherwise-do-this)
    IfElse(Box<Expr>, Box<Expr>, Box<Expr>),
    /// '(3 (if (+ 3 3) 4 5) 7)
    Quote(Vec<Expr>),
}

/// We now wrap this type and a few other primitives into our Atom type.
/// Remember from before that Atoms form one half of our language.
#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum Atom {
    Num(i32),
    Keyword(String),
    Boolean(bool),
    BuiltIn(BuiltIn),
}

/// Now, the most basic type. We define some built-in functions that our lisp has
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) enum BuiltIn {
    Plus,
    Minus,
    Times,
    Divide,
    Equal,
    Not,
}

/// With types defined, we move onto the top-level expression parser!
fn parse_expr(i: &mut &'_ str) -> ModalResult<Expr> {
    preceded(
        multispace0,
        alt((parse_constant, parse_application, parse_if, parse_quote)),
    )
    .parse_next(i)
}

/// We then add the Expr layer on top
fn parse_constant(i: &mut &'_ str) -> ModalResult<Expr> {
    parse_atom.map(Expr::Constant).parse_next(i)
}

/// Now we take all these simple parsers and connect them.
/// We can now parse half of our language!
fn parse_atom(i: &mut &'_ str) -> ModalResult<Atom> {
    alt((
        parse_num,
        parse_bool,
        parse_builtin.map(Atom::BuiltIn),
        parse_keyword,
    ))
    .parse_next(i)
}

/// Next up is number parsing. We're keeping it simple here by accepting any number (> 1)
/// of digits but ending the program if it doesn't fit into an i32.
fn parse_num(i: &mut &'_ str) -> ModalResult<Atom> {
    alt((
        digit1.try_map(|digit_str: &str| digit_str.parse::<i32>().map(Atom::Num)),
        preceded("-", digit1).map(|digit_str: &str| Atom::Num(-digit_str.parse::<i32>().unwrap())),
    ))
    .parse_next(i)
}

/// Our boolean values are also constant, so we can do it the same way
fn parse_bool(i: &mut &'_ str) -> ModalResult<Atom> {
    alt((
        "#t".map(|_| Atom::Boolean(true)),
        "#f".map(|_| Atom::Boolean(false)),
    ))
    .parse_next(i)
}

fn parse_builtin(i: &mut &'_ str) -> ModalResult<BuiltIn> {
    // alt gives us the result of first parser that succeeds, of the series of
    // parsers we give it
    alt((
        parse_builtin_op,
        // map lets us process the parsed output, in this case we know what we parsed,
        // so we ignore the input and return the BuiltIn directly
        "not".map(|_| BuiltIn::Not),
    ))
    .parse_next(i)
}

/// Continuing the trend of starting from the simplest piece and building up,
/// we start by creating a parser for the built-in operator functions.
fn parse_builtin_op(i: &mut &'_ str) -> ModalResult<BuiltIn> {
    // one_of matches one of the characters we give it
    let t = one_of(['+', '-', '*', '/', '=']).parse_next(i)?;

    // because we are matching single character tokens, we can do the matching logic
    // on the returned value
    Ok(match t {
        '+' => BuiltIn::Plus,
        '-' => BuiltIn::Minus,
        '*' => BuiltIn::Times,
        '/' => BuiltIn::Divide,
        '=' => BuiltIn::Equal,
        _ => unreachable!(),
    })
}

/// The next easiest thing to parse are keywords.
/// We introduce some error handling combinators: `context` for human readable errors
/// and `cut_err` to prevent back-tracking.
///
/// Put plainly: `preceded(":", cut_err(alpha1))` means that once we see the `:`
/// character, we have to see one or more alphabetic characters or the input is invalid.
fn parse_keyword(i: &mut &'_ str) -> ModalResult<Atom> {
    preceded(":", cut_err(alpha1))
        .context(StrContext::Label("keyword"))
        .map(|sym_str: &str| Atom::Keyword(sym_str.to_owned()))
        .parse_next(i)
}

/// We can now use our new combinator to define the rest of the `Expr`s.
///
/// Starting with function application, we can see how the parser mirrors our data
/// definitions: our definition is `Application(Box<Expr>, Vec<Expr>)`, so we know
/// that we need to parse an expression and then parse 0 or more expressions, all
/// wrapped in an S-expression.
///
/// tuples are themselves a parser, used to sequence parsers together, so we can translate this
/// directly and then map over it to transform the output into an `Expr::Application`
fn parse_application(i: &mut &'_ str) -> ModalResult<Expr> {
    let application_inner = (parse_expr, repeat(0.., parse_expr))
        .map(|(head, tail)| Expr::Application(Box::new(head), tail));
    // finally, we wrap it in an s-expression
    s_exp(application_inner).parse_next(i)
}

/// Because `Expr::If` and `Expr::IfElse` are so similar (we easily could have
/// defined `Expr::If` to have an `Option` for the else block), we parse both
/// in a single function.
///
/// In fact, we define our parser as if `Expr::If` was defined with an Option in it,
/// we have the `opt` combinator which fits very nicely here.
fn parse_if(i: &mut &'_ str) -> ModalResult<Expr> {
    let if_inner = preceded(
        // here to avoid ambiguity with other names starting with `if`, if we added
        // variables to our language, we say that if must be terminated by at least
        // one whitespace character
        terminated("if", multispace1),
        cut_err((parse_expr, parse_expr, opt(parse_expr))),
    )
    .map(|(pred, true_branch, maybe_false_branch)| {
        if let Some(false_branch) = maybe_false_branch {
            Expr::IfElse(
                Box::new(pred),
                Box::new(true_branch),
                Box::new(false_branch),
            )
        } else {
            Expr::If(Box::new(pred), Box::new(true_branch))
        }
    })
    .context(StrContext::Label("if expression"));
    s_exp(if_inner).parse_next(i)
}

/// A quoted S-expression is list data structure.
///
/// This example doesn't have the symbol atom, but by adding variables and changing
/// the definition of quote to not always be around an S-expression, we'd get them
/// naturally.
fn parse_quote(i: &mut &'_ str) -> ModalResult<Expr> {
    // this should look very straight-forward after all we've done:
    // we find the `'` (quote) character, use cut_err to say that we're unambiguously
    // looking for an s-expression of 0 or more expressions, and then parse them
    preceded("'", cut_err(s_exp(repeat(0.., parse_expr))))
        .context(StrContext::Label("quote"))
        .map(Expr::Quote)
        .parse_next(i)
}

/// Before continuing, we need a helper function to parse lists.
/// A list starts with `(` and ends with a matching `)`.
/// By putting whitespace and newline parsing here, we can avoid having to worry about it
/// in much of the rest of the parser.
//.parse_next/
/// Unlike the previous functions, this function doesn't take or consume input, instead it
/// takes a parsing function and returns a new parsing function.
fn s_exp<'a, O1, F>(inner: F) -> impl ModalParser<&'a str, O1, ContextError>
where
    F: ModalParser<&'a str, O1, ContextError>,
{
    delimited(
        '(',
        preceded(multispace0, inner),
        cut_err(preceded(multispace0, ')')).context(StrContext::Label("closing paren")),
    )
}

/// And that's it!
/// We can now parse our entire lisp language.
///
/// But in order to make it a little more interesting, we can hack together
/// a little interpreter to take an Expr, which is really an
/// [Abstract Syntax Tree](https://en.wikipedia.org/wiki/Abstract_syntax_tree) (AST),
/// and give us something back
///
/// This function tries to reduce the AST.
/// This has to return an Expression rather than an Atom because quoted `s_expressions`
/// can't be reduced
fn eval_expression(e: Expr) -> Option<Expr> {
    match e {
        // Constants and quoted s-expressions are our base-case
        Expr::Constant(_) | Expr::Quote(_) => Some(e),
        // we then recursively `eval_expression` in the context of our special forms
        // and built-in operators
        Expr::If(pred, true_branch) => {
            let reduce_pred = eval_expression(*pred)?;
            if get_bool_from_expr(reduce_pred)? {
                eval_expression(*true_branch)
            } else {
                None
            }
        }
        Expr::IfElse(pred, true_branch, false_branch) => {
            let reduce_pred = eval_expression(*pred)?;
            if get_bool_from_expr(reduce_pred)? {
                eval_expression(*true_branch)
            } else {
                eval_expression(*false_branch)
            }
        }
        Expr::Application(head, tail) => {
            let reduced_head = eval_expression(*head)?;
            let reduced_tail = tail
                .into_iter()
                .map(eval_expression)
                .collect::<Option<Vec<Expr>>>()?;
            if let Expr::Constant(Atom::BuiltIn(bi)) = reduced_head {
                Some(Expr::Constant(match bi {
                    BuiltIn::Plus => Atom::Num(
                        reduced_tail
                            .into_iter()
                            .map(get_num_from_expr)
                            .collect::<Option<Vec<i32>>>()?
                            .into_iter()
                            .sum(),
                    ),
                    BuiltIn::Times => Atom::Num(
                        reduced_tail
                            .into_iter()
                            .map(get_num_from_expr)
                            .collect::<Option<Vec<i32>>>()?
                            .into_iter()
                            .product(),
                    ),
                    BuiltIn::Equal => Atom::Boolean(
                        reduced_tail
                            .iter()
                            .zip(reduced_tail.iter().skip(1))
                            .all(|(a, b)| a == b),
                    ),
                    BuiltIn::Not => {
                        if reduced_tail.len() != 1 {
                            return None;
                        } else {
                            Atom::Boolean(!get_bool_from_expr(
                                reduced_tail.first().cloned().unwrap(),
                            )?)
                        }
                    }
                    BuiltIn::Minus => {
                        Atom::Num(if let Some(first_elem) = reduced_tail.first().cloned() {
                            let fe = get_num_from_expr(first_elem)?;
                            reduced_tail
                                .into_iter()
                                .map(get_num_from_expr)
                                .collect::<Option<Vec<i32>>>()?
                                .into_iter()
                                .skip(1)
                                .fold(fe, |a, b| a - b)
                        } else {
                            Default::default()
                        })
                    }
                    BuiltIn::Divide => {
                        Atom::Num(if let Some(first_elem) = reduced_tail.first().cloned() {
                            let fe = get_num_from_expr(first_elem)?;
                            reduced_tail
                                .into_iter()
                                .map(get_num_from_expr)
                                .collect::<Option<Vec<i32>>>()?
                                .into_iter()
                                .skip(1)
                                .fold(fe, |a, b| a / b)
                        } else {
                            Default::default()
                        })
                    }
                }))
            } else {
                None
            }
        }
    }
}

/// To start we define a couple of helper functions
fn get_num_from_expr(e: Expr) -> Option<i32> {
    if let Expr::Constant(Atom::Num(n)) = e {
        Some(n)
    } else {
        None
    }
}

fn get_bool_from_expr(e: Expr) -> Option<bool> {
    if let Expr::Constant(Atom::Boolean(b)) = e {
        Some(b)
    } else {
        None
    }
}
