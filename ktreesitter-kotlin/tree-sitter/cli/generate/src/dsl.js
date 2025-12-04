function alias(rule, value) {
  const result = {
    type: "ALIAS",
    content: normalize(rule),
    named: false,
    value: null
  };

  switch (value.constructor) {
    case String:
      result.named = false;
      result.value = value;
      return result;
    case ReferenceError:
      result.named = true;
      result.value = value.symbol.name;
      return result;
    case Object:
    case GrammarSymbol:
      if (typeof value.type === 'string' && value.type === 'SYMBOL') {
        result.named = true;
        result.value = value.name;
        return result;
      }
  }

  throw new Error(`Invalid alias value ${value}`);
}

function blank() {
  return {
    type: "BLANK"
  };
}

function field(name, rule) {
  return {
    type: "FIELD",
    name,
    content: normalize(rule)
  }
}

function choice(...elements) {
  return {
    type: "CHOICE",
    members: elements.map(normalize)
  };
}

function optional(value) {
  checkArguments(arguments, arguments.length, optional, 'optional');
  return choice(value, blank());
}

function prec(number, rule) {
  checkPrecedence(number);
  checkArguments(
    arguments,
    arguments.length - 1,
    prec,
    'prec',
    ' and a precedence argument'
  );

  return {
    type: "PREC",
    value: number,
    content: normalize(rule)
  };
}

prec.left = function(number, rule) {
  if (rule == null) {
    rule = number;
    number = 0;
  }

  checkPrecedence(number);
  checkArguments(
    arguments,
    arguments.length - 1,
    prec.left,
    'prec.left',
    ' and an optional precedence argument'
  );

  return {
    type: "PREC_LEFT",
    value: number,
    content: normalize(rule)
  };
}

prec.right = function(number, rule) {
  if (rule == null) {
    rule = number;
    number = 0;
  }

  checkPrecedence(number);
  checkArguments(
    arguments,
    arguments.length - 1,
    prec.right,
    'prec.right',
    ' and an optional precedence argument'
  );

  return {
    type: "PREC_RIGHT",
    value: number,
    content: normalize(rule)
  };
}

prec.dynamic = function(number, rule) {
  checkPrecedence(number);
  checkArguments(
    arguments,
    arguments.length - 1,
    prec.dynamic,
    'prec.dynamic',
    ' and a precedence argument'
  );

  return {
    type: "PREC_DYNAMIC",
    value: number,
    content: normalize(rule)
  };
}

function repeat(rule) {
  checkArguments(arguments, arguments.length, repeat, 'repeat');
  return {
    type: "REPEAT",
    content: normalize(rule)
  };
}

function repeat1(rule) {
  checkArguments(arguments, arguments.length, repeat1, 'repeat1');
  return {
    type: "REPEAT1",
    content: normalize(rule)
  };
}

function seq(...elements) {
  return {
    type: "SEQ",
    members: elements.map(normalize)
  };
}

class GrammarSymbol {
  constructor(name) {
    this.type = "SYMBOL";
    this.name = name;
  }
}

function reserved(wordset, rule) {
  if (typeof wordset !== 'string') {
    throw new Error('Invalid reserved word set name: ' + wordset)
  }
  return {
    type: "RESERVED",
    content: normalize(rule),
    context_name: wordset,
  }
}

function sym(name) {
  return new GrammarSymbol(name);
}

function token(value) {
  checkArguments(arguments, arguments.length, token, 'token', '', 'literal');
  return {
    type: "TOKEN",
    content: normalize(value)
  };
}

token.immediate = function(value) {
  checkArguments(arguments, arguments.length, token.immediate, 'token.immediate', '', 'literal');
  return {
    type: "IMMEDIATE_TOKEN",
    content: normalize(value)
  };
}

function normalize(value) {
  if (typeof value == "undefined")
    throw new Error("Undefined symbol");

  switch (value.constructor) {
    case String:
      return {
        type: 'STRING',
        value
      };
    case RegExp:
      return value.flags ? {
        type: 'PATTERN',
        value: value.source,
        flags: value.flags
      } : {
        type: 'PATTERN',
        value: value.source
      };
    case RustRegex:
      return {
        type: 'PATTERN',
        value: value.value
      };
    case ReferenceError:
      throw value
    default:
      if (typeof value.type === 'string') {
        return value;
      } else {
        throw new TypeError(`Invalid rule: ${value}`);
      }
  }
}

function RuleBuilder(ruleMap) {
  return new Proxy({}, {
    get(_, propertyName) {
      const symbol = sym(propertyName);

      if (!ruleMap || Object.prototype.hasOwnProperty.call(ruleMap, propertyName)) {
        return symbol;
      } else {
        const error = new ReferenceError(`Undefined symbol '${propertyName}'`);
        error.symbol = symbol;
        return error;
      }
    }
  })
}

function grammar(baseGrammar, options) {
  let inherits = undefined;

  if (!options) {
    options = baseGrammar;
    baseGrammar = {
      name: null,
      rules: {},
      extras: [normalize(/\s/)],
      conflicts: [],
      externals: [],
      inline: [],
      supertypes: [],
      precedences: [],
      reserved: {},
    };
  } else {
    baseGrammar = baseGrammar.grammar;
    inherits = baseGrammar.name;
  }

  let externals = baseGrammar.externals;
  if (options.externals) {
    if (typeof options.externals !== "function") {
      throw new Error("Grammar's 'externals' property must be a function.");
    }

    const externalsRuleBuilder = RuleBuilder(null)
    const externalRules = options.externals.call(externalsRuleBuilder, externalsRuleBuilder, baseGrammar.externals);

    if (!Array.isArray(externalRules)) {
      throw new Error("Grammar's 'externals' property must return an array of rules.");
    }

    externals = externalRules.map(normalize);
  }

  const ruleMap = {};
  for (const key of Object.keys(options.rules)) {
    ruleMap[key] = true;
  }
  for (const key of Object.keys(baseGrammar.rules)) {
    ruleMap[key] = true;
  }
  for (const external of externals) {
    if (typeof external.name === 'string') {
      ruleMap[external.name] = true;
    }
  }

  const ruleBuilder = RuleBuilder(ruleMap);

  const name = options.name;
  if (typeof name !== "string") {
    throw new Error("Grammar's 'name' property must be a string.");
  }

  if (!/^[a-zA-Z_]\w*$/.test(name)) {
    throw new Error("Grammar's 'name' property must not start with a digit and cannot contain non-word characters.");
  }

  if (inherits && typeof inherits !== "string") {
    throw new Error("Base grammar's 'name' property must be a string.");
  }

  if (inherits && !/^[a-zA-Z_]\w*$/.test(name)) {
    throw new Error("Base grammar's 'name' property must not start with a digit and cannot contain non-word characters.");
  }

  const rules = Object.assign({}, baseGrammar.rules);
  if (options.rules) {
    if (typeof options.rules !== "object") {
      throw new Error("Grammar's 'rules' property must be an object.");
    }

    for (const ruleName of Object.keys(options.rules)) {
      const ruleFn = options.rules[ruleName];
      if (typeof ruleFn !== "function") {
        throw new Error(`Grammar rules must all be functions. '${ruleName}' rule is not.`);
      }
      const rule = ruleFn.call(ruleBuilder, ruleBuilder, baseGrammar.rules[ruleName]);
      if (rule === undefined) {
        throw new Error(`Rule '${ruleName}' returned undefined.`);
      }
      rules[ruleName] = normalize(rule);
    }
  }

  let reserved = baseGrammar.reserved;
  if (options.reserved) {
    if (typeof options.reserved !== "object") {
      throw new Error("Grammar's 'reserved' property must be an object.");
    }

    for (const reservedWordSetName of Object.keys(options.reserved)) {
      const reservedWordSetFn = options.reserved[reservedWordSetName]
      if (typeof reservedWordSetFn !== "function") {
        throw new Error(`Grammar reserved word sets must all be functions. '${reservedWordSetName}' is not.`);
      }

      const reservedTokens = reservedWordSetFn.call(ruleBuilder, ruleBuilder, baseGrammar.reserved[reservedWordSetName]);

      if (!Array.isArray(reservedTokens)) {
        throw new Error(`Grammar's reserved word set functions must all return arrays of rules. '${reservedWordSetName}' does not.`);
      }

      reserved[reservedWordSetName] = reservedTokens.map(normalize);
    }
  }

  let extras = baseGrammar.extras.slice();
  if (options.extras) {
    if (typeof options.extras !== "function") {
      throw new Error("Grammar's 'extras' property must be a function.");
    }

    extras = options.extras
      .call(ruleBuilder, ruleBuilder, baseGrammar.extras)

    if (!Array.isArray(extras)) {
      throw new Error("Grammar's 'extras' function must return an array.")
    }

    extras = extras.map(normalize);
  }

  let word = baseGrammar.word;
  if (options.word) {
    word = options.word.call(ruleBuilder, ruleBuilder).name;
    if (typeof word != 'string') {
      throw new Error("Grammar's 'word' property must be a named rule.");
    }

    if (word === 'ReferenceError') {
      throw new Error("Grammar's 'word' property must be a valid rule name.");
    }
  }

  let conflicts = baseGrammar.conflicts;
  if (options.conflicts) {
    if (typeof options.conflicts !== "function") {
      throw new Error("Grammar's 'conflicts' property must be a function.");
    }

    const baseConflictRules = baseGrammar.conflicts.map(conflict => conflict.map(sym));
    const conflictRules = options.conflicts.call(ruleBuilder, ruleBuilder, baseConflictRules);

    if (!Array.isArray(conflictRules)) {
      throw new Error("Grammar's conflicts must be an array of arrays of rules.");
    }

    conflicts = conflictRules.map(conflictSet => {
      if (!Array.isArray(conflictSet)) {
        throw new Error("Grammar's conflicts must be an array of arrays of rules.");
      }

      return conflictSet.map(symbol => normalize(symbol).name);
    });
  }

  let inline = baseGrammar.inline;
  if (options.inline) {
    if (typeof options.inline !== "function") {
      throw new Error("Grammar's 'inline' property must be a function.");
    }

    const baseInlineRules = baseGrammar.inline.map(sym);
    const inlineRules = options.inline.call(ruleBuilder, ruleBuilder, baseInlineRules);

    if (!Array.isArray(inlineRules)) {
      throw new Error("Grammar's inline must be an array of rules.");
    }

    inline = inlineRules.filter((symbol, index, self) => {
      if (self.findIndex(s => s.name === symbol.name) !== index) {
        console.log(`Warning: duplicate inline rule '${symbol.name}'`);
        return false;
      }
      if (symbol.name === 'ReferenceError') {
        console.log(`Warning: inline rule '${symbol.symbol.name}' is not defined.`);
        return false;
      }
      return true;
    }).map(symbol => symbol.name);
  }

  let supertypes = baseGrammar.supertypes;
  if (options.supertypes) {
    if (typeof options.supertypes !== "function") {
      throw new Error("Grammar's 'supertypes' property must be a function.");
    }

    const baseSupertypeRules = baseGrammar.supertypes.map(sym);
    const supertypeRules = options.supertypes.call(ruleBuilder, ruleBuilder, baseSupertypeRules);

    if (!Array.isArray(supertypeRules)) {
      throw new Error("Grammar's supertypes must be an array of rules.");
    }

    supertypes = supertypeRules.map(symbol => {
      if (symbol.name === 'ReferenceError') {
        throw new Error(`Supertype rule \`${symbol.symbol.name}\` is not defined.`);
      }
      return symbol.name;
    });
  }

  let precedences = baseGrammar.precedences;
  if (options.precedences) {
    if (typeof options.precedences !== "function") {
      throw new Error("Grammar's 'precedences' property must be a function");
    }
    precedences = options.precedences.call(ruleBuilder, ruleBuilder, baseGrammar.precedences);
    if (!Array.isArray(precedences)) {
      throw new Error("Grammar's precedences must be an array of arrays of rules.");
    }
    precedences = precedences.map(list => {
      if (!Array.isArray(list)) {
        throw new Error("Grammar's precedences must be an array of arrays of rules.");
      }
      return list.map(normalize);
    });
  }

  if (Object.keys(rules).length === 0) {
    throw new Error("Grammar must have at least one rule.");
  }

  return {
    grammar: {
      name,
      inherits,
      word,
      rules,
      extras,
      conflicts,
      precedences,
      externals,
      inline,
      supertypes,
      reserved,
    },
  };
}

class RustRegex {
  constructor(value) {
    this.value = value;
  }
}

function checkArguments(args, ruleCount, caller, callerName, suffix = '', argType = 'rule') {
  // Allow for .map() usage where additional arguments are index and the entire array.
  const isMapCall = ruleCount === 3 && typeof args[1] === 'number' && Array.isArray(args[2]);
  if (isMapCall) {
    ruleCount = typeof args[2] === 'number' ? 1 : args[2].length;
  }
  if (ruleCount > 1 && !isMapCall) {
    const error = new Error([
      `The \`${callerName}\` function only takes one ${argType} argument${suffix}.`,
      `You passed in multiple ${argType}s. Did you mean to call \`seq\`?\n`
    ].join('\n'));
    Error.captureStackTrace(error, caller);
    throw error
  }
}

function checkPrecedence(value) {
  if (value == null) {
    throw new Error('Missing precedence value');
  }
}

function getEnv(name) {
  if (globalThis.process) return process.env[name]; // Node/Bun
  if (globalThis.Deno) return Deno.env.get(name); // Deno
  throw Error("Unsupported JS runtime");
}

globalThis.alias = alias;
globalThis.blank = blank;
globalThis.choice = choice;
globalThis.optional = optional;
globalThis.prec = prec;
globalThis.repeat = repeat;
globalThis.repeat1 = repeat1;
globalThis.reserved = reserved;
globalThis.seq = seq;
globalThis.sym = sym;
globalThis.token = token;
globalThis.grammar = grammar;
globalThis.field = field;
globalThis.RustRegex = RustRegex;

const result = await import(getEnv("TREE_SITTER_GRAMMAR_PATH"));
const object = {
  "$schema": "https://tree-sitter.github.io/tree-sitter/assets/schemas/grammar.schema.json",
  ...(result.default?.grammar ?? result.grammar)
};
const output = JSON.stringify(object);

if (globalThis.process) { // Node/Bun
  process.stdout.write(output);
} else if (globalThis.Deno) { // Deno
  Deno.stdout.writeSync(new TextEncoder().encode(output));
} else {
  throw Error("Unsupported JS runtime");
}
