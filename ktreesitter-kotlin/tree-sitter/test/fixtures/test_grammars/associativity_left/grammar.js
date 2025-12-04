module.exports = grammar({
    name: 'associativity_left',

    rules: {
        expression: $ => choice(
            $.math_operation,
            $.identifier
        ),

        math_operation: $ => prec.left(seq(
            $.expression,
            '+',
            $.expression,
        )),

        identifier: $ => /[a-z]+/,
    }
});