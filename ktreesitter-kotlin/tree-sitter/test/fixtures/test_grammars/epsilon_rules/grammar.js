module.exports = grammar({
    name: 'epsilon_rules',

    rules: {
        rule_1: $ => $.rule_2,

        rule_2: $ => optional($.rule_3),

        rule_3: $ => 'x'
    }
});