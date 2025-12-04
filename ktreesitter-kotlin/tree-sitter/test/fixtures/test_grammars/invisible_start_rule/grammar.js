module.exports = grammar({
  name: "invisible_start_rule",
  rules: {
    _value: $ => choice($.a, $.b),
    a: $ => "a",
    b: $ => "b",
  },
});
