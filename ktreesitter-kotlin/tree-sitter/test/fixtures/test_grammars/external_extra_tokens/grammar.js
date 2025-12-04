module.exports = grammar({
    name: "external_extra_tokens",

    externals: $ => [
        $.comment
    ],

    extras: $ => [/\s/, $.comment],

    rules: {
        assignment: $ => seq($.variable, '=', $.variable),
        variable: $ => /[a-z]+/
    }
})
