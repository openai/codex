module.exports = grammar({
  name: "inlined_aliased_rules",

  extras: $ => [/\s/],

  inline: $ => [$.expression],

  rules: {
    statement: $ => seq($.expression, ";"),

    expression: $ =>
      choice(
        $.call_expression,
        $.member_expression,
        alias($.identifier, $.variable_name),
      ),

    call_expression: $ => prec.left(seq($.expression, "(", $.expression, ")")),

    member_expression: $ =>
      prec.left(
        1,
        seq($.expression, ".", alias($.identifier, $.property_name)),
      ),

    identifier: $ => /[a-z]+/,
  },
});
