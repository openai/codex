// This grammar has a conflict that involves *repeat rules*: auxiliary rules that are added by the
// parser generator in order to implement repetition. There is no way of referring to these rules in
// the grammar DSL, so these conflicts must be resolved by referring to their parent rules.

module.exports = grammar({
    name: 'conflict_in_repeat_rule',

    rules: {
        statement: $ => choice(
            seq($.array, ';'),
            seq($.array_type, $.identifier, ';'),
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