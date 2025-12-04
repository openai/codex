module.exports = grammar({
  name: 'named_rule_aliased_as_anonymous',

  rules: {
    a: $ => seq(
      alias($.b, 'the-alias'),
      $.c,
      $.b,
    ),

    b: _ => 'B',

    c: _ => 'C',
  },
});
