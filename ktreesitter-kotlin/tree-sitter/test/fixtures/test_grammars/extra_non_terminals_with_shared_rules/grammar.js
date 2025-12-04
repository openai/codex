// This grammar has a non-terminal extra rule `macro_statement` that contains
// child rules that are also used elsewhere in the grammar.

module.exports = grammar({
  name: "extra_non_terminals_with_shared_rules",

  extras: $ => [/\s+/, $.macro_statement],

  rules: {
    program: $ => repeat($.statement),
    statement: $ => seq(repeat($.label_declaration), ';'),
    macro_statement: $ => seq('%', $.statement),
    label_declaration: $ => seq($.identifier, ':'),
    identifier: $ => /[a-zA-Z]+/
  }
})