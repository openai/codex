module.exports = grammar({
  name: 'precedence_on_single_child_positive',

  rules: {
    expression: $ => choice($.function_call, $.identifier),

    function_call: $ => prec.right(1, choice(
      seq($.identifier, $.expression),
      seq($.identifier, $.block),
      seq($.identifier, $.expression, $.block),
    )),

    block: $ => seq('{', $.expression, '}'),

    identifier: _ => /[a-zA-X]+/,
  },
});
