module.exports = grammar({
    name: 'aliases_in_root',

    extras: $ => [
      /\s/,
      $.comment,
    ],

    rules: {
        document: $ => seq(
          alias($.foo, $.bar),
          $.foo,
        ),

        foo: $ => "foo",

        comment: $ => /#.*/
    }
});
