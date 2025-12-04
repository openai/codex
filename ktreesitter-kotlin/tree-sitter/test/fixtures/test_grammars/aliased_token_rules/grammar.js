// This grammar shows that `ALIAS` rules can be applied directly to `TOKEN` and `IMMEDIATE_TOKEN`
// rules.

module.exports = grammar({
    name: 'aliased_token_rules',

    extras: $ => [/\s/],

    rules: {
        expression: $ => seq(
            'a',
            alias(token(seq('b', 'c')), $.X),
            alias(token.immediate(seq('d', 'e')), $.Y),
        ),
    }
});