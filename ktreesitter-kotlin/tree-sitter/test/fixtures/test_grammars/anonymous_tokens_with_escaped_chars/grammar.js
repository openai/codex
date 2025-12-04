// Every token in a grammar is given a name in the generated parser. Anonymous tokens (tokens
// specified directly in the body of some larger rule) are named according their content. So when
// tokens contains characters that aren't valid in a C string literal, we need to escape those
// characters. This grammar tests that this escaping works. The test is basically that the generated
// parser compiles successfully.

module.exports = grammar({
    name: "anonymous_tokens_with_escaped_chars",
    rules: {
        first_rule: $ => choice(
            "\n",
            "\r\n",
            "'hello'",
            /\d+/,
        )
    }
})
