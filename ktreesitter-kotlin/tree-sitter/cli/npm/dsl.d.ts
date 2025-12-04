type AliasRule = { type: 'ALIAS'; named: boolean; content: Rule; value: string };
type BlankRule = { type: 'BLANK' };
type ChoiceRule = { type: 'CHOICE'; members: Rule[] };
type FieldRule = { type: 'FIELD'; name: string; content: Rule };
type ImmediateTokenRule = { type: 'IMMEDIATE_TOKEN'; content: Rule };
type PatternRule = { type: 'PATTERN'; value: string };
type PrecDynamicRule = { type: 'PREC_DYNAMIC'; content: Rule; value: number };
type PrecLeftRule = { type: 'PREC_LEFT'; content: Rule; value: number };
type PrecRightRule = { type: 'PREC_RIGHT'; content: Rule; value: number };
type PrecRule = { type: 'PREC'; content: Rule; value: number };
type Repeat1Rule = { type: 'REPEAT1'; content: Rule };
type RepeatRule = { type: 'REPEAT'; content: Rule };
type ReservedRule = { type: 'RESERVED'; content: Rule; context_name: string };
type SeqRule = { type: 'SEQ'; members: Rule[] };
type StringRule = { type: 'STRING'; value: string };
type SymbolRule<Name extends string> = { type: 'SYMBOL'; name: Name };
type TokenRule = { type: 'TOKEN'; content: Rule };

type Rule =
  | AliasRule
  | BlankRule
  | ChoiceRule
  | FieldRule
  | ImmediateTokenRule
  | PatternRule
  | PrecDynamicRule
  | PrecLeftRule
  | PrecRightRule
  | PrecRule
  | Repeat1Rule
  | RepeatRule
  | SeqRule
  | StringRule
  | SymbolRule<string>
  | TokenRule;

class RustRegex {
  value: string;

  constructor(pattern: string) {
    this.value = pattern;
  }
}

type RuleOrLiteral = Rule | RegExp | RustRegex | string;

type GrammarSymbols<RuleName extends string> = {
  [name in RuleName]: SymbolRule<name>;
} &
  Record<string, SymbolRule<string>>;

type RuleBuilder<RuleName extends string> = (
  $: GrammarSymbols<RuleName>,
  previous?: Rule,
) => RuleOrLiteral;

type RuleBuilders<
  RuleName extends string,
  BaseGrammarRuleName extends string
> = {
    [name in RuleName]: RuleBuilder<RuleName | BaseGrammarRuleName>;
  };

interface Grammar<
  RuleName extends string,
  BaseGrammarRuleName extends string = never,
  Rules extends RuleBuilders<RuleName, BaseGrammarRuleName> = RuleBuilders<
    RuleName,
    BaseGrammarRuleName
  >
> {
  /**
   * Name of the grammar language.
   */
  name: string;

  /** Mapping of grammar rule names to rule builder functions. */
  rules: Rules;

  /**
   * An array of arrays of precedence names or rules. Each inner array represents
   * a *descending* ordering. Names/rules listed earlier in one of these arrays
   * have higher precedence than any names/rules listed later in the same array.
   *
   * Using rules is just a shorthand way for using a name then calling prec()
   * with that name. It is just a convenience.
   */
  precedences?: (
    $: GrammarSymbols<RuleName | BaseGrammarRuleName>,
    previous: Rule[][],
  ) => RuleOrLiteral[][],

  /**
   * An array of arrays of rule names. Each inner array represents a set of
   * rules that's involved in an _LR(1) conflict_ that is _intended to exist_
   * in the grammar. When these conflicts occur at runtime, Tree-sitter will
   * use the GLR algorithm to explore all of the possible interpretations. If
   * _multiple_ parses end up succeeding, Tree-sitter will pick the subtree
   * whose corresponding rule has the highest total _dynamic precedence_.
   *
   * @param $ grammar rules
   */
  conflicts?: (
    $: GrammarSymbols<RuleName | BaseGrammarRuleName>,
    previous: Rule[][],
  ) => RuleOrLiteral[][];

  /**
   * An array of token names which can be returned by an _external scanner_.
   * External scanners allow you to write custom C code which runs during the
   * lexing process in order to handle lexical rules (e.g. Python's indentation
   * tokens) that cannot be described by regular expressions.
   *
   * @param $ grammar rules
   * @param previous array of externals from the base schema, if any
   *
   * @see https://tree-sitter.github.io/tree-sitter/creating-parsers/4-external-scanners
   */
  externals?: (
    $: Record<string, SymbolRule<string>>,
    previous: Rule[],
  ) => RuleOrLiteral[];

  /**
   * An array of tokens that may appear anywhere in the language. This
   * is often used for whitespace and comments. The default value of
   * extras is to accept whitespace. To control whitespace explicitly,
   * specify extras: `$ => []` in your grammar.
   *
   *  @param $ grammar rules
   */
  extras?: (
    $: GrammarSymbols<RuleName | BaseGrammarRuleName>,
  ) => RuleOrLiteral[];

  /**
   * An array of rules that should be automatically removed from the
   * grammar by replacing all of their usages with a copy of their definition.
   * This is useful for rules that are used in multiple places but for which
   * you don't want to create syntax tree nodes at runtime.
   *
   * @param $ grammar rules
   */
  inline?: (
    $: GrammarSymbols<RuleName | BaseGrammarRuleName>,
    previous: Rule[],
  ) => RuleOrLiteral[];

  /**
   * A list of hidden rule names that should be considered supertypes in the
   * generated node types file.
   *
   * @param $ grammar rules
   *
   * @see https://tree-sitter.github.io/tree-sitter/using-parsers/6-static-node-types
   */
  supertypes?: (
    $: GrammarSymbols<RuleName | BaseGrammarRuleName>,
    previous: Rule[],
  ) => RuleOrLiteral[];

  /**
   * The name of a token that will match keywords for the purpose of the
   * keyword extraction optimization.
   *
   * @param $ grammar rules
   *
   * @see https://tree-sitter.github.io/tree-sitter/creating-parsers/3-writing-the-grammar#keyword-extraction
   */
  word?: ($: GrammarSymbols<RuleName | BaseGrammarRuleName>) => RuleOrLiteral;


  /**
   * Mapping of names to reserved word sets. The first reserved word set is the
   * global word set, meaning it applies to every rule in every parse state.
   * The other word sets can be used with the `reserved` function.
   */
  reserved?: Record<
    string,
    ($: GrammarSymbols<RuleName | BaseGrammarRuleName>) => RuleOrLiteral[]
  >;
}

type GrammarSchema<RuleName extends string> = {
  [K in keyof Grammar<RuleName>]: K extends 'rules'
  ? Record<RuleName, Rule>
  : Grammar<RuleName>[K];
};

/**
 * Causes the given rule to appear with an alternative name in the syntax tree.
 * For instance with `alias($.foo, 'bar')`, the aliased rule will appear as an
 * anonymous node, as if the rule had been written as the simple string.
 *
 * @param rule rule that will be aliased
 * @param name target name for the alias
 */
declare function alias(rule: RuleOrLiteral, name: string): AliasRule;

/**
 * Causes the given rule to appear as an alternative named node, for instance
 * with `alias($.foo, $.bar)`, the aliased rule `foo` will appear as a named
 * node called `bar`.
 *
 * @param rule rule that will be aliased
 * @param symbol target symbol for the alias
 */
declare function alias(
  rule: RuleOrLiteral,
  symbol: SymbolRule<string>,
): AliasRule;

/**
 * Creates a blank rule, matching nothing.
 */
declare function blank(): BlankRule;

/**
 * Assigns a field name to the child node(s) matched by the given rule.
 * In the resulting syntax tree, you can then use that field name to
 * access specific children.
 *
 * @param name name of the field
 * @param rule rule the field should match
 */
declare function field(name: string, rule: RuleOrLiteral): FieldRule;

/**
 * Creates a rule that matches one of a set of possible rules. The order
 * of the arguments does not matter. This is analogous to the `|` (pipe)
 * operator in EBNF notation.
 *
 * @param options possible rule choices
 */
declare function choice(...options: RuleOrLiteral[]): ChoiceRule;

/**
 * Creates a rule that matches zero or one occurrence of a given rule.
 * It is analogous to the `[x]` (square bracket) syntax in EBNF notation.
 *
 * @param value rule to be made optional
 */
declare function optional(rule: RuleOrLiteral): ChoiceRule;

/**
 * Marks the given rule with a precedence which will be used to resolve LR(1)
 * conflicts at parser-generation time. When two rules overlap in a way that
 * represents either a true ambiguity or a _local_ ambiguity given one token
 * of lookahead, Tree-sitter will try to resolve the conflict by matching the
 * rule with the higher precedence.
 *
 * Precedence values can either be strings or numbers. When comparing rules
 * with numerical precedence, higher numbers indicate higher precedences. To
 * compare rules with string precedence, Tree-sitter uses the grammar's `precedences`
 * field.
 *
 * rules is zero. This works similarly to the precedence directives in Yacc grammars.
 *
 * @param value precedence weight
 * @param rule rule being weighted
 *
 * @see https://en.wikipedia.org/wiki/LR_parser#Conflicts_in_the_constructed_tables
 * @see https://docs.oracle.com/cd/E19504-01/802-5880/6i9k05dh3/index.html
 */
declare const prec: {
  (value: String | number, rule: RuleOrLiteral): PrecRule;

  /**
   * Marks the given rule as left-associative (and optionally applies a
   * numerical precedence). When an LR(1) conflict arises in which all of the
   * rules have the same numerical precedence, Tree-sitter will consult the
   * rules' associativity. If there is a left-associative rule, Tree-sitter
   * will prefer matching a rule that ends _earlier_. This works similarly to
   * associativity directives in Yacc grammars.
   *
   * @param value (optional) precedence weight
   * @param rule rule to mark as left-associative
   *
   * @see https://docs.oracle.com/cd/E19504-01/802-5880/6i9k05dh3/index.html
   */
  left(rule: RuleOrLiteral): PrecLeftRule;
  left(value: String | number, rule: RuleOrLiteral): PrecLeftRule;

  /**
   * Marks the given rule as right-associative (and optionally applies a
   * numerical precedence). When an LR(1) conflict arises in which all of the
   * rules have the same numerical precedence, Tree-sitter will consult the
   * rules' associativity. If there is a right-associative rule, Tree-sitter
   * will prefer matching a rule that ends _later_. This works similarly to
   * associativity directives in Yacc grammars.
   *
   * @param value (optional) precedence weight
   * @param rule rule to mark as right-associative
   *
   * @see https://docs.oracle.com/cd/E19504-01/802-5880/6i9k05dh3/index.html
   */
  right(rule: RuleOrLiteral): PrecRightRule;
  right(value: String | number, rule: RuleOrLiteral): PrecRightRule;

  /**
   * Marks the given rule with a numerical precedence which will be used to
   * resolve LR(1) conflicts at _runtime_ instead of parser-generation time.
   * This is only necessary when handling a conflict dynamically using the
   * `conflicts` field in the grammar, and when there is a genuine _ambiguity_:
   * multiple rules correctly match a given piece of code. In that event,
   * Tree-sitter compares the total dynamic precedence associated with each
   * rule, and selects the one with the highest total. This is similar to
   * dynamic precedence directives in Bison grammars.
   *
   * @param value precedence weight
   * @param rule rule being weighted
   *
   * @see https://www.gnu.org/software/bison/manual/html_node/Generalized-LR-Parsing.html
   */
  dynamic(value: String | number, rule: RuleOrLiteral): PrecDynamicRule;
};

/**
 * Creates a rule that matches _zero-or-more_ occurrences of a given rule.
 * It is analogous to the `{x}` (curly brace) syntax in EBNF notation. This
 * rule is implemented in terms of `repeat1` but is included because it
 * is very commonly used.
 *
 * @param rule rule to repeat, zero or more times
 */
declare function repeat(rule: RuleOrLiteral): RepeatRule;

/**
 * Creates a rule that matches one-or-more occurrences of a given rule.
 *
 * @param rule rule to repeat, one or more times
 */
declare function repeat1(rule: RuleOrLiteral): Repeat1Rule;

/**
 * Overrides the global reserved word set for a given rule. The word set name
 * should be defined in the `reserved` field in the grammar.
 *
 * @param wordset name of the reserved word set
 * @param rule rule that will use the reserved word set
 */
declare function reserved(wordset: string, rule: RuleOrLiteral): ReservedRule;

/**
 * Creates a rule that matches any number of other rules, one after another.
 * It is analogous to simply writing multiple symbols next to each other
 * in EBNF notation.
 *
 * @param rules ordered rules that comprise the sequence
 */
declare function seq(...rules: RuleOrLiteral[]): SeqRule;

/**
 * Creates a symbol rule, representing another rule in the grammar by name.
 *
 * @param name name of the target rule
 */
declare function sym<Name extends string>(name: Name): SymbolRule<Name>;

/**
 * Marks the given rule as producing only a single token. Tree-sitter's
 * default is to treat each String or RegExp literal in the grammar as a
 * separate token. Each token is matched separately by the lexer and
 * returned as its own leaf node in the tree. The token function allows
 * you to express a complex rule using the DSL functions (rather
 * than as a single regular expression) but still have Tree-sitter treat
 * it as a single token.
 *
 * @param rule rule to represent as a single token
 */
declare const token: {
  (rule: RuleOrLiteral): TokenRule;

  /**
   * Marks the given rule as producing an immediate token. This allows
   * the parser to produce a different token based on whether or not
   * there are `extras` preceding the token's main content. When there
   * are _no_ leading `extras`, an immediate token is preferred over a
   * normal token which would otherwise match.
   *
   * @param rule rule to represent as an immediate token
   */
  immediate(rule: RuleOrLiteral): ImmediateTokenRule;
};

/**
 * Creates a new language grammar with the provided schema.
 *
 * @param options grammar options
 */
declare function grammar<RuleName extends string>(
  options: Grammar<RuleName>,
): GrammarSchema<RuleName>;

/**
 * Extends an existing language grammar with the provided options,
 * creating a new language.
 *
 * @param baseGrammar base grammar schema to extend from
 * @param options grammar options for the new extended language
 */
declare function grammar<
  BaseGrammarRuleName extends string,
  RuleName extends string
>(
  baseGrammar: GrammarSchema<BaseGrammarRuleName>,
  options: Grammar<RuleName, BaseGrammarRuleName>,
): GrammarSchema<RuleName | BaseGrammarRuleName>;
