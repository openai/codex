module.exports = grammar({
  name: 'get_col_should_hang_not_crash',

  externals: $ => [
    $.test,
  ],

  rules: {
    source_file: $ => seq(
      $.test
    ),
  },
});
