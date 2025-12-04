// This grammar shows that `ALIAS` rules can *contain* a rule that is marked as `inline`. It also
// shows that you can alias a rule that would otherwise be anonymous, and it will then appear as a
// named node.

module.exports = grammar({
    name: 'aliased_inlined_rules',

    extras: $ => [/\s/],

    inline: $ => [$.identifier],

    rules: {
        statement: $ => seq($._expression, ';'),

        _expression: $ => choice(
            $.member_expression,
            alias($.identifier, $.variable_name),
        ),

        member_expression: $ => prec.left(1, seq(
            $._expression,
            '.',
            alias($.identifier, $.property_name)
        )),

        identifier: $ => choice('a', 'b', 'c')
    }
});