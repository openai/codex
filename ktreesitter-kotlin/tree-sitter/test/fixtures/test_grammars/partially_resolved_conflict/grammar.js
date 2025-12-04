module.exports = grammar({
  name: 'partially_resolved_conflict',

  rules: {
    expression: $ => choice($.binary, $.identifier),

    unary_a: $ => prec(2, seq('!', $.expression)),

    unary_b: $ => prec(2, seq('!', $.expression)),

    binary: $ => seq(
      choice($.unary_a, $.unary_b, $.expression),
      '<',
      $.expression,
    ),

    identifier: _ => /[a-z]+/,
  },
});
