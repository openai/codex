module.exports = grammar({
  name: "external_unicode_column_alignment",

  externals: $ => [
    $._start_list,
    $.list_item,
    $._end_list
  ],

  extras: $ => [/\s/, '□'],

  rules: {
    expression: $ => repeat($.list),
    
    list: $ => seq($._start_list, repeat1($.list_item), $._end_list)
  }
})
