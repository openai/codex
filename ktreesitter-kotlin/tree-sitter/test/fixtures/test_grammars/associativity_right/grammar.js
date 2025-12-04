module.exports = grammar({
    name: 'associativity_right',

    rules: {
        expression: $ => choice(
            $.math_operation,
            $.identifier
        ),

        math_operation: $ => prec.right(seq(
            $.expression,
            '+',
            $.expression,
        )),

        identifier: $ => /[a-z]+/,
    }
});