// This grammar has an external scanner whose `scan` method needs to be able to check for the
// validity of an *internal* token. This is done by including the names of that internal token
// (`line_break`) in the grammar's `externals` field.

module.exports = grammar({
    name: 'external_and_internal_tokens',

    externals: $ => [
        $.string,
        $.line_break,
    ],

    extras: $ => [/\s/],

    rules: {
        statement: $ => seq(
            $._expression,
            $._expression,
            $.line_break,
        ),

        _expression: $ => choice(
            $.string,
            $.variable,
            $.number,
        ),

        variable: $ => /[a-z]+/,
        number: $ => /\d+/,
        line_break: $ => '\n',
    }
});