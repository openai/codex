module.exports = grammar({
    name: 'aliased_rules',

    extras: $ => [
      /\s/,
      $.star,
    ],

    rules: {
        statement: $ => seq($._expression, ';'),

        _expression: $ => choice(
            $.call_expression,
            $.member_expression,
            alias($.identifier, $.variable_name),
        ),

        call_expression: $ => prec.left(seq(
            $._expression,
            '(',
            $._expression,
            ')'
        )),

        member_expression: $ => prec.left(1, seq(
            $._expression,
            '.',
            alias($.identifier, $.property_name)
        )),

        identifier: $ => /[a-z]+/,

        // Tests for https://github.com/tree-sitter/tree-sitter/issues/1834
        //
        // Even though the alias is unused, that issue causes all instances of
        // the extra that appear in the tree to be renamed to `star_aliased`.
        //
        // Instead, this alias should have no effect because it is unused.
        star: $ => '*',
        unused: $ => alias($.star, $.star_aliased),
    }
});
