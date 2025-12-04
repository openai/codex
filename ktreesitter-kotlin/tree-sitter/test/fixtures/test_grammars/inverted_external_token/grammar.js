module.exports = grammar({
  name: "inverted_external_token",

  externals: $ => [$.line_break],

  extras: $ => [/\s/],

  rules: {
    program: $ => repeat($.statement),
    statement: $ => seq($._expression, $.line_break),
    _expression: $ => choice($.identifier, $.member_expression),
    member_expression: $ => prec.left(seq($._expression, ".", $.identifier)),
    identifier: $ => /[a-z]+/,
  },
});
