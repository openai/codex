module.exports = grammar({
  name: 'start_rule_is_blank',

  rules: {
    first_rule: _ => blank(),
  },
});
