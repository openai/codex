module.exports = grammar({
  name: 'unused_rules',

  rules: {
    a: $ => seq($.d, $.h),

    b: _ => 'B',

    c: _ => 'C',

    d: $ => seq($.e, $.f),

    e: _ => 'E',

    f: _ => 'F',

    g: _ => 'G',

    h: $ => seq($.i, $.j),

    i: _ => 'I',

    j: _ => 'J',

    k: _ => 'K',
  },
});
