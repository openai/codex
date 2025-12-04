// This grammar demonstrates the usage of the IMMEDIATE_TOKEN rule. It allows the parser to produce
// a different token based on whether or not there are `extras` preceding the token's main content.
// When there are *no* leading `extras`, an immediate token is preferred over a normal token which
// would otherwise match.

module.exports = grammar({
  name: "immediate_tokens",

  extras: $ => [/\s/],

  rules: {
    program: $ => $._expression,

    _expression: $ => choice(
      $.call,
      $.infix,
      $.prefix,
      $.identifier,
    ),

    call: $ => prec.left(-1, seq(
      $._expression,
      $._expression,
    )),

    prefix: $ => seq(
      '::',
      $.identifier,
    ),

    infix: $ => seq(
      $._expression,
      token.immediate('::'),
      $.identifier,
    ),

    identifier: $ => /[a-z]+/
  }
})
