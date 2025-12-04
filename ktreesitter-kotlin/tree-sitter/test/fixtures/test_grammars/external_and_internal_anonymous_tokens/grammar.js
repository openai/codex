module.exports = grammar({
    name: 'external_and_internal_anonymous_tokens',

    externals: $ => [
        $.string,
        '\n'
    ],

    extras: $ => [/\s/],

    rules: {
        statement: $ => seq(
            $._expression,
            $._expression,
            '\n'
        ),

        _expression: $ => choice(
            $.string,
            $.variable,
            $.number
        ),

        variable: $ => /[a-z]+/,

        number: $ => /\d+/
    }
})