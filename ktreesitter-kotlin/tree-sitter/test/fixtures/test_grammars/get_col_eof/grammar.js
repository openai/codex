module.exports = grammar({
  name: "get_col_eof",

  externals: $ => [
    $.char
  ],

  rules: {
    source_file: $ => repeat($.char),
  }
});
