module.exports = grammar({
  name: "next_sibling_from_zwt",
  extras: $ => [
    /\s|\\\r?\n/,
  ],

  rules: {
    source: $ => seq(
      'a',
      $._bc,
      'd',
      'e',
      'f',
    ),

    _bc: $ => seq(
      'b',
      'c',
    ),
  }
});
