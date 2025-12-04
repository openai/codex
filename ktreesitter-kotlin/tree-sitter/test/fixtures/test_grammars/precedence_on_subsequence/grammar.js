module.exports = grammar({
  name: 'precedence_on_subsequence',

  rules: {
    expression: $ => prec.left(choice(
      $.function_call,
      $.identifier,
      $.scope_resolution,
    )),

    function_call: $ => choice(
      seq($.identifier, $.expression),
      prec(1, seq($.identifier, $.block)),
      prec(-1, seq($.identifier, $.do_block)),
      seq($.identifier, prec(1, seq($.expression, $.block))),
      seq($.identifier, prec(-1, seq($.expression, $.do_block))),
    ),

    scope_resolution: $ => prec.left(1, choice(
      seq($.expression, '::', $.expression),
      seq('::', $.expression),
    )),

    block: _ => '{}',

    do_block: _ => 'do end',

    identifier: _ => /[a-zA-Z]+/,
  },
});
