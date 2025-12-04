// Normally, when there are invisible rules (rules whose names start with an `_`) that simply wrap
// another rule, there is an optimization at parser-generation time called *Unit Reduction
// Elimination* that avoids creating nodes for those rules at runtime. One case where this
// optimization must *not* be applied is when those invisible rules are going to be aliased within
// their parent rule. In that situation, eliminating the invisible node could cause the alias to be
// incorrectly applied to its child.

module.exports = grammar({
    name: 'aliased_unit_reductions',

    extras: $ => [/\s/],

    rules: {
        statement: $ => seq(
            $._a,

            // The `_b` rule is always aliased to `b_prime`, so it is internally treated
            // as a simple alias.
            alias($._b, $.b_prime),

            // The `_c` rule is used without an alias in addition to being aliased to `c_prime`,
            // so it is not a simple alias.
            alias($._c, $.c_prime),

            $._c,
            ';'
        ),

        _a: $ => $._A,
        _b: $ => $._B,
        _c: $ => $._C,
        _A: $ => $.identifier,
        _B: $ => $.identifier,
        _C: $ => $.identifier,

        identifier: $ => /[a-z]+/,
    }
});