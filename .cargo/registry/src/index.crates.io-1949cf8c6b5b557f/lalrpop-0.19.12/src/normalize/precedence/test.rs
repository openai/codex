use crate::parser;
use crate::test_util::compare;

use super::expand_precedence;
use super::resolve::resolve;

#[test]
fn multilevel() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    Expr: u32 = {
       #[precedence(level="1")]
       <left:Expr> "*" <right:Expr> => 0,
       #[precedence(level="1")]
       <left:Expr> "/" <right:Expr> => 0,
       #[precedence(level="2")]
       <left:Expr> "+" <right:Expr> => 0,
       #[precedence(level="2")]
       <left:Expr> "-" <right:Expr> => 0,
       #[precedence(level="3")]
       <left:Expr> "%" <right:Expr> => 0,
    }

    Ext: u32 = Expr;
"#,
    )
    .unwrap();

    let expected = parser::parse_grammar(
        r#"
grammar;
    Expr1: u32 = {
       <left:Expr1> "*" <right:Expr1> => 0,
       <left:Expr1> "/" <right:Expr1> => 0,
    }

    Expr2: u32 = {
       <left:Expr2> "+" <right:Expr2> => 0,
       <left:Expr2> "-" <right:Expr2> => 0,
       Expr1,
    }

    Expr: u32 = {
       <left:Expr> "%" <right:Expr> => 0,
       Expr2,
    }

    Ext: u32 = Expr;
"#,
    )
    .unwrap();

    compare(expand_precedence(grammar), resolve(expected));
}

#[test]
fn with_assoc() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    Expr: u32 = {
       #[precedence(level="1")]
       "const" => 0,
       "!" <Expr> => 0,

       #[precedence(level="2")] #[assoc(side="none")]
       "!!" <Expr> => 0,

       #[assoc(side="left")]
       "const2" => 0,

       #[precedence(level="2")] #[assoc(side="left")]
       <left:Expr> "*" <right:Expr> => 0,

       #[assoc(side="right")]
       <left:Expr> "/" <right:Expr> => 0,

       #[precedence(level="3")] #[assoc(side="left")]
       <left:Expr> "?" <middle:Expr> ":" <right:Expr> => 0,

       #[assoc(side="right")]
       <left:Expr> "|" <middle:Expr> "-" <right:Expr> => 0,

       #[assoc(side="none")]
       <left:Expr> "^" <middle:Expr> "$" <right:Expr> => 0,

       #[assoc(side="all")]
       <left:Expr> "[" <middle:Expr> ";" <right:Expr> => 0,
   }
"#,
    )
    .unwrap();

    let expected = parser::parse_grammar(
        r#"
grammar;
    Expr1: u32 = {
        "const" => 0,
        "!" <Expr1> => 0,
    }

    Expr2: u32 = {
        "!!" <Expr1> => 0,
        "const2" => 0,
        <left:Expr2> "*" <right:Expr1> => 0,
        <left:Expr1> "/" <right:Expr2> => 0,
        Expr1,
    }

    Expr: u32 = {
       <left:Expr> "?" <middle:Expr2> ":" <right:Expr2> => 0,
       <left:Expr2> "|" <middle:Expr2> "-" <right:Expr> => 0,
       <left:Expr2> "^" <middle:Expr2> "$" <right:Expr2> => 0,
       <left:Expr> "[" <middle:Expr> ";" <right:Expr> => 0,
       Expr2,
    }
"#,
    )
    .unwrap();

    compare(expand_precedence(grammar), resolve(expected));
}

#[test]
fn non_consecutive_levels() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    Expr: u32 = {
       #[precedence(level="5")] #[assoc(side="left")]
       <left:Expr> "?" <middle:Expr> ":" <right:Expr> => 0,

       #[assoc(side="right")]
       <left:Expr> "|" <middle:Expr> "-" <right:Expr> => 0,

       #[assoc(side="none")]
       <left:Expr> "^" <middle:Expr> "$" <right:Expr> => 0,

       #[assoc(side="all")]
       <left:Expr> "[" <middle:Expr> ";" <right:Expr> => 0,

       #[precedence(level="0")]
       "const" => 0,
       "!" <Expr> => 0,

       #[precedence(level="3")]
       #[assoc(side="none")]
       "!!" <Expr> => 0,

       #[assoc(side="left")]
       "const2" => 0,
       <left:Expr> "*" <right:Expr> => 0,

       #[assoc(side="right")]
       <left:Expr> "/" <right:Expr> => 0,

          }
"#,
    )
    .unwrap();

    let expected = parser::parse_grammar(
        r#"
grammar;
    Expr0: u32 = {
        "const" => 0,
        "!" <Expr0> => 0,
    }

    Expr3: u32 = {
        "!!" <Expr0> => 0,
        "const2" => 0,
        <left:Expr3> "*" <right:Expr0> => 0,
        <left:Expr0> "/" <right:Expr3> => 0,
        Expr0,
    }

    Expr: u32 = {
       <left:Expr> "?" <middle:Expr3> ":" <right:Expr3> => 0,
       <left:Expr3> "|" <middle:Expr3> "-" <right:Expr> => 0,
       <left:Expr3> "^" <middle:Expr3> "$" <right:Expr3> => 0,
       <left:Expr> "[" <middle:Expr> ";" <right:Expr> => 0,
       Expr3,
    }
"#,
    )
    .unwrap();

    compare(expand_precedence(grammar), resolve(expected));
}

#[test]
fn macros() {
    let grammar = parser::parse_grammar(
        r#"
grammar;
    Expr: u32 = {
      #[precedence(level="1")]
      "const" => 0,
      #[precedence(level="2")]
      #[assoc(side="left")]
      MacroOp<OpTimes, Expr, Expr> => 0,
      #[precedence(level="3")]
      #[assoc(side="right")]
      MacroOp<OpPlus, Expr, Expr> => 0,
    }

    MacroOp<Op, RuleLeft, RuleRight>: u32 = <left: RuleLeft> <op: Op> <right: RuleRight> => 0;

    OpTimes: () = "*" => ();
    OpPlus: () = "+" => ();

    Ext: u32 = Expr;
"#,
    )
    .unwrap();

    let expected = parser::parse_grammar(
        r#"
grammar;
    Expr1: u32 = {
      "const" => 0,
    }

    Expr2: u32 = {
       MacroOp<OpTimes, Expr2, Expr1> => 0,
       Expr1,
    }

    Expr: u32 = {
       MacroOp<OpPlus, Expr2, Expr> => 0,
       Expr2,
    }

    MacroOp<Op, RuleLeft, RuleRight>: u32 = <left: RuleLeft> <op: Op> <right: RuleRight> => 0;

    OpTimes: () = "*" => ();
    OpPlus: () = "+" => ();

    Ext: u32 = Expr;
"#,
    )
    .unwrap();

    compare(expand_precedence(grammar), resolve(expected));
}

#[test]
fn calculator() {
    let grammar = parser::parse_grammar(
        r#"
grammar;

Expr: i32 = {
    #[precedence(lvl="0")]
    Num,
    "(" <Expr> ")",

    #[precedence(lvl="1")] #[assoc(side="left")]
    <l:Expr> "*" <r:Expr> => l * r,
    <l:Expr> "/" <r:Expr> => l / r,

    #[precedence(lvl="2")] #[assoc(side="left")]
    <l:Expr> "+" <r:Expr> => l + r,
    <l:Expr> "-" <r:Expr> => l - r,
};

Num: i32 = {
    r"[0-9]+" => i32::from_str(<>).unwrap(),
};
"#,
    )
    .unwrap();

    let expected = parser::parse_grammar(
        r#"
grammar;

    Expr0: i32 = {
        Num,
        "(" <Expr0> ")",
    }

    Expr1: i32 = {
        <l:Expr1> "*" <r:Expr0> => l * r,
        <l:Expr1> "/" <r:Expr0> => l / r,
        Expr0,
    };

    Expr: i32 = {
        <l:Expr> "+" <r:Expr1> => l + r,
        <l:Expr> "-" <r:Expr1> => l - r,
        Expr1,
    };

    Num: i32 = {
        r"[0-9]+" => i32::from_str(<>).unwrap(),
    };
"#,
    )
    .unwrap();

    compare(expand_precedence(grammar), resolve(expected));
}
