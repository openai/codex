module.exports = grammar({
    name: 'associativity_missing',

    rules: {
        expression: $ => choice(
            $.math_operation,
            $.identifier
        ),

        math_operation: $ => seq(
            $.expression,
            '+',
            $.expression,
        ),

        identifier: $ => /[a-z]+/,
    }
});