module.exports = grammar({
  name: "depends_on_column",
  rules: {
    x_is_at: ($) => seq(/[ \r\n]*/, choice($.odd_column, $.even_column), "x"),
  },
  externals: ($) => [$.odd_column, $.even_column],
});
