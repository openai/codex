module.exports = grammar({
    name: 'dynamic_precedence',

    extras: $ => [/\s/],

    conflicts: $ => [[$.expression, $.type]],

    rules: {
        program: $ => choice(
            $.declaration,
            $.expression,
        ),

        expression: $ => choice(
            prec.left(seq($.expression, '*', $.expression)),
            $.identifier
        ),

        declaration: $ => seq(
            $.type,
            $.declarator,
        ),

        declarator: $ => choice(
            prec.dynamic(1, seq('*', $.identifier)),
            $.identifier,
        ),

        type: $ => $.identifier,
        identifier: $ => /[a-z-A-Z]+/
    }
});