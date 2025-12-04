// This grammar uses an external scanner to match special string literals,
// that track the nesting depth of parentheses, similar to Ruby's percent
// string literals.

module.exports = grammar({
  name: "external_tokens",

  externals: $ => [
    $._percent_string,
    $._percent_string_start,
    $._percent_string_end,
  ],

  extras: $ => [/\s/],

  rules: {
    expression: $ => choice($.string, $.sum, $.identifier),

    sum: $ => prec.left(seq($.expression, '+', $.expression)),

    string: $ => choice($._percent_string, seq(
      $._percent_string_start,
      $.expression,
      $._percent_string_end,
    )),

    identifier: $ => /[a-z]+/
  }
})
