module.exports = grammar({
    name: 'conflicting_precedence',

    rules: {
        expression: $ => choice(
            $.sum,
            $.product,
            $.other_thing,
        ),

        sum: $ => prec.left(0, seq($.expression, '+', $.expression)),
        product: $ => prec.left(1, seq($.expression, '*', $.expression)),
        other_thing: $ => prec.left(-1, seq($.expression, '*', '*')),
        identifier: $ => /[a-zA-Z]+/
    }
});