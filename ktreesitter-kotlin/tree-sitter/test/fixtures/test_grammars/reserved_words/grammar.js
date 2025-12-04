const RESERVED_NAMES = ["if", "while", "var"];
const RESERVED_PROPERTY_NAMES = ["var"];

module.exports = grammar({
  name: "reserved_words",

  reserved: {
    global: $ => RESERVED_NAMES,
    property: $ => RESERVED_PROPERTY_NAMES,
  },

  word: $ => $.identifier,

  rules: {
    program: $ => repeat($._statement),

    block: $ => seq("{", repeat($._statement), "}"),

    _statement: $ => choice(
      $.var_declaration,
      $.if_statement,
      $.while_statement,
      $.expression_statement,
    ),

    var_declaration: $ => seq("var", $.identifier, "=", $._expression, ";"),

    if_statement: $ => seq("if", $.parenthesized_expression, $.block),

    while_statement: $ => seq("while", $.parenthesized_expression, $.block),

    expression_statement: $ => seq($._expression, ";"),

    _expression: $ => choice(
      $.identifier,
      $.parenthesized_expression,
      $.call_expression,
      $.member_expression,
      $.object,
      $.regex,
    ),

    parenthesized_expression: $ => seq("(", $._expression, ")"),

    member_expression: $ => seq($._expression, ".", $.identifier),

    call_expression: $ => seq($._expression, "(", repeat(seq($._expression, ",")), ")"),

    object: $ => seq("{", repeat(seq(choice($.pair, $.getter), ",")), "}"),

    regex: $ => seq('/', $.regex_pattern, '/'),

    regex_pattern: $ => token(prec(-1, /[^/\n]+/)),

    pair: $ => seq(reserved('property', $.identifier), ":", $._expression),

    getter: $ => seq(
      "get",
      reserved('property', $.identifier),
      "(",
      ")",
      $.block,
    ),

    identifier: $ => /[a-z_]\w*/,
  },
});
