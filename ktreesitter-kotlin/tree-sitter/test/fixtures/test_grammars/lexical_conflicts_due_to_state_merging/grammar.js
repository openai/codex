module.exports = grammar({
  name: 'lexical_conflicts_due_to_state_merging',

  rules: {
    expression: $ => choice(
      $.conditional,
      $.quotient,
      $.regex,
      $.number,
      $.parenthesized,
    ),

    conditional: $ => prec.left(1, seq(
      'if',
      $.parenthesized,
      $.expression
    )),

    quotient: $ => prec.left(seq(
      $.expression,
      '/',
      $.expression
    )),

    regex: $ => /\/[^/\n]+\//,

    number: $ => /\d+/,

    parenthesized: $ => seq('(', $.expression, ')'),
  },
});
