module.exports = grammar({
  name: 'nested_inlined_rules',

  inline: $ => [
    $.top_level_item,
    $.statement,
  ],

  rules: {
    program: $ => repeat1($.top_level_item),

    top_level_item: $ => choice($.statement, '!'),

    statement: $ => choice($.expression_statement, $.return_statement),

    return_statement: $ => seq('return', $.number, ';'),

    expression_statement: $ => seq($.number, ';'),

    number: _ => /\d+/,
  },
});
