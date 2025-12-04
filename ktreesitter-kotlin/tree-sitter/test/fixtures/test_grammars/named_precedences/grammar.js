module.exports = grammar({
  name: 'named_precedences',

  conflicts: $ => [
    [$.expression, $.type],
    [$.expression, $.nested_type],
  ],

  precedences: $ => [
    [$.member_expression, "and", "or"],
    [$.nested_type, "type_intersection", "type_union"],
  ],

  rules: {
    program: $ => repeat(choice(
      $.expression_statement,
      $.declaration_statement,
    )),

    expression_statement: $ => seq($.expression, ';'),

    declaration_statement: $ => seq($.type, $.expression, ';'),

    expression: $ => choice(
      $.member_expression,
      $.binary_expression,
      $.identifier,
    ),

    member_expression: $ => seq($.expression, '.', $.identifier),

    binary_expression: $ => choice(
      prec.left('or', seq($.expression, '||', $.expression)),
      prec.left('and', seq($.expression, '&&', $.expression)),
    ),

    type: $ => choice($.nested_type, $.binary_type, $.identifier),

    nested_type: $ => seq($.identifier, '.', $.identifier),

    binary_type: $ => choice(
      prec.left('type_union', seq($.type, '||', $.type)),
      prec.left('type_intersection', seq($.type, '&&', $.type)),
    ),

    identifier: $ => /[a-z]\w+/,
  },
});
