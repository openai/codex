module.exports = grammar({
  name: 'readme_grammar',

  // Things that can appear anywhere in the language, like comments
  // and whitespace, are expressed as 'extras'.
  extras: $ => [
    /\s/,
    $.comment,
  ],

  rules: {
    // The first rule listed in the grammar becomes the 'start rule'.
    expression: $ => choice(
      $.sum,
      $.product,
      $.number,
      $.variable,
      seq('(', $.expression, ')'),
    ),

    // Tokens like '+' and '*' are described directly within the
    // grammar's rules, as opposed to in a separate lexer description.
    sum: $ => prec.left(1, seq($.expression, '+', $.expression)),

    // Ambiguities can be resolved at compile time by assigning precedence
    // values to rule subtrees.
    product: $ => prec.left(2, seq($.expression, '*', $.expression)),

    // Tokens can be specified using ECMAScript regexps.
    number: _ => /\d+/,

    comment: _ => /#.*/,

    variable: _ => new RustRegex('(?i:[a-z])\\w*'),
  },
});
