// This grammar is similar to the `conflict_in_repeat_rule` grammar, except that the conflict occurs
// after an external token is consumed. This tests that the logic for determining the repeat rule's
// "parent" rule works in the presence of external tokens.

module.exports = grammar({
    name: 'conflict_in_repeat_rule_after_external_token',

    externals: $ => [
        $._program_start,
    ],

    rules: {
        statement: $ => choice(
            seq($._program_start, $.array, ';'),
            seq($._program_start, $.array_type, $.identifier, ';'),
        ),

        array: $ => seq(
            '[',
            repeat(choice($.identifier, '0')),
            ']',
        ),

        array_type: $ => seq(
            '[',
            repeat(choice($.identifier, 'void')),
            ']',
        ),

        identifier: $ => /[a-z]+/
    }
});