module.exports = grammar({
  name: "inline_rules",

  extras: $ => [/\s/],

  inline: $ => [$.expression],

  rules: {
    program: $ => repeat1($.statement),
    statement: $ => seq($.expression, ";"),
    expression: $ => choice(
      $.sum,
      $.product,
      $.number,
      $.parenthesized_expression,
    ),
    parenthesized_expression: $ => seq("(", $.expression, ")"),
    sum: $ => prec.left(seq($.expression, "+", $.expression)),
    product: $ => prec.left(2, seq($.expression, "*", $.expression)),
    number: $ => /\d+/,
  }
})
