module.exports = grammar({
  name: 'precedence_on_token',

  extras: $ => [
    /\s/,
    $.comment,
  ],

  rules: {
    program: $ => repeat(choice(
      $.string,
      $.regex,
      $.identifier,
      $.slash,
    )),

    comment: _ => token(prec(1, /\/\/.*|\/\*[^*]*\*\//)),

    string: $ => seq(
      '"',
      repeat(choice(
        token(prec(2, /[^\"\n\\]+/)),
        $.escape_sequence,
      )),
      '"',
    ),

    escape_sequence: _ => /\\./,

    regex: _ => /\/[^\/\n]+\/[a-z]*/,

    identifier: _ => /[a-z]\w*/,

    slash: _ => '/',
  },
});
