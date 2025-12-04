module.exports = grammar({
  name: 'uses_current_column',

  externals: $ => [
    $._indent,
    $._dedent,
    $._newline,
  ],

  rules: {
    block: $ => repeat1($._statement),

    _statement: $ => seq($._expression, $._newline),

    _expression: $ => choice(
      $.do_expression,
      $.binary_expression,
      $.identifier,
    ),

    do_expression: $ => seq(
      'do',
      $._indent,
      $.block,
      $._dedent,
    ),

    binary_expression: $ => prec.left(1, seq(
      $._expression,
      choice('=', '+', '-'),
      $._expression,
    )),

    identifier: _ => /\w+/,
  },
});
