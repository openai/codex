module.exports = grammar({
  name: 'unicode_classes',

  rules: {
    program: $ => repeat(choice(
      $.lower,
      $.upper,
      $.math_sym,
      $.letter_number,
    )),

    lower: _ => /\p{Ll}\p{L}*/,

    upper: _ => /\p{Lu}\p{L}*/,

    math_sym: _ => /\p{Sm}+/,

    letter_number: _ => /\p{Letter_Number}/,
  },
});
