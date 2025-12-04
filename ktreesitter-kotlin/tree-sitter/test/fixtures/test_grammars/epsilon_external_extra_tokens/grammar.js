module.exports = grammar({
    name: 'epsilon_external_extra_tokens',

    extras: $ => [/\s/, $.comment],

    externals: $ => [$.comment],

    rules: {
        document: $ => seq('a', 'b'),
    }
});
