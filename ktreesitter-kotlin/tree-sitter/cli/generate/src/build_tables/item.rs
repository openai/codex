use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    sync::LazyLock,
};

use crate::{
    grammars::{
        LexicalGrammar, Production, ProductionStep, ReservedWordSetId, SyntaxGrammar,
        NO_RESERVED_WORDS,
    },
    rules::{Associativity, Precedence, Symbol, SymbolType, TokenSet},
};

static START_PRODUCTION: LazyLock<Production> = LazyLock::new(|| Production {
    dynamic_precedence: 0,
    steps: vec![ProductionStep {
        symbol: Symbol {
            index: 0,
            kind: SymbolType::NonTerminal,
        },
        precedence: Precedence::None,
        associativity: None,
        alias: None,
        field_name: None,
        reserved_word_set_id: NO_RESERVED_WORDS,
    }],
});

/// A [`ParseItem`] represents an in-progress match of a single production in a grammar.
#[derive(Clone, Copy, Debug)]
pub struct ParseItem<'a> {
    /// The index of the parent rule within the grammar.
    pub variable_index: u32,
    /// The number of symbols that have already been matched.
    pub step_index: u32,
    /// The production being matched.
    pub production: &'a Production,
    /// A boolean indicating whether any of the already-matched children were
    /// hidden nodes and had fields. Ordinarily, a parse item's behavior is not
    /// affected by the symbols of its preceding children; it only needs to
    /// keep track of their fields and aliases.
    ///
    /// Take for example these two items:
    ///   X -> a b • c
    ///   X -> a g • c
    ///
    /// They can be considered equivalent, for the purposes of parse table
    /// generation, because they entail the same actions. But if this flag is
    /// true, then the item's set of inherited fields may depend on the specific
    /// symbols of its preceding children.
    pub has_preceding_inherited_fields: bool,
}

/// A [`ParseItemSet`] represents a set of in-progress matches of productions in a
/// grammar, and for each in-progress match, a set of "lookaheads" - tokens that
/// are allowed to *follow* the in-progress rule. This object corresponds directly
/// to a state in the final parse table.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ParseItemSet<'a> {
    pub entries: Vec<ParseItemSetEntry<'a>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseItemSetEntry<'a> {
    pub item: ParseItem<'a>,
    pub lookaheads: TokenSet,
    pub following_reserved_word_set: ReservedWordSetId,
}

/// A [`ParseItemSetCore`] is like a [`ParseItemSet`], but without the lookahead
/// information. Parse states with the same core are candidates for merging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseItemSetCore<'a> {
    pub entries: Vec<ParseItem<'a>>,
}

pub struct ParseItemDisplay<'a>(
    pub &'a ParseItem<'a>,
    pub &'a SyntaxGrammar,
    pub &'a LexicalGrammar,
);

pub struct TokenSetDisplay<'a>(
    pub &'a TokenSet,
    pub &'a SyntaxGrammar,
    pub &'a LexicalGrammar,
);

pub struct ParseItemSetDisplay<'a>(
    pub &'a ParseItemSet<'a>,
    pub &'a SyntaxGrammar,
    pub &'a LexicalGrammar,
);

impl<'a> ParseItem<'a> {
    pub fn start() -> Self {
        ParseItem {
            variable_index: u32::MAX,
            production: &START_PRODUCTION,
            step_index: 0,
            has_preceding_inherited_fields: false,
        }
    }

    pub fn step(&self) -> Option<&'a ProductionStep> {
        self.production.steps.get(self.step_index as usize)
    }

    pub fn symbol(&self) -> Option<Symbol> {
        self.step().map(|step| step.symbol)
    }

    pub fn associativity(&self) -> Option<Associativity> {
        self.prev_step().and_then(|step| step.associativity)
    }

    pub fn precedence(&self) -> &Precedence {
        self.prev_step()
            .map_or(&Precedence::None, |step| &step.precedence)
    }

    pub fn prev_step(&self) -> Option<&'a ProductionStep> {
        if self.step_index > 0 {
            Some(&self.production.steps[self.step_index as usize - 1])
        } else {
            None
        }
    }

    #[must_use]
    pub fn is_done(&self) -> bool {
        self.step_index as usize == self.production.steps.len()
    }

    #[must_use]
    pub const fn is_augmented(&self) -> bool {
        self.variable_index == u32::MAX
    }

    /// Create an item like this one, but advanced by one step.
    #[must_use]
    pub const fn successor(&self) -> Self {
        ParseItem {
            variable_index: self.variable_index,
            production: self.production,
            step_index: self.step_index + 1,
            has_preceding_inherited_fields: self.has_preceding_inherited_fields,
        }
    }

    /// Create an item identical to this one, but with a different production.
    /// This is used when dynamically "inlining" certain symbols in a production.
    pub const fn substitute_production(&self, production: &'a Production) -> Self {
        let mut result = *self;
        result.production = production;
        result
    }
}

impl<'a> ParseItemSet<'a> {
    pub fn insert(&mut self, item: ParseItem<'a>) -> &mut ParseItemSetEntry<'a> {
        match self.entries.binary_search_by(|e| e.item.cmp(&item)) {
            Err(i) => {
                self.entries.insert(
                    i,
                    ParseItemSetEntry {
                        item,
                        lookaheads: TokenSet::new(),
                        following_reserved_word_set: ReservedWordSetId::default(),
                    },
                );
                &mut self.entries[i]
            }
            Ok(i) => &mut self.entries[i],
        }
    }

    pub fn core(&self) -> ParseItemSetCore<'a> {
        ParseItemSetCore {
            entries: self.entries.iter().map(|e| e.item).collect(),
        }
    }
}

impl fmt::Display for ParseItemDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.0.is_augmented() {
            write!(f, "START →")?;
        } else {
            write!(
                f,
                "{} →",
                self.1.variables[self.0.variable_index as usize].name
            )?;
        }

        for (i, step) in self.0.production.steps.iter().enumerate() {
            if i == self.0.step_index as usize {
                write!(f, " •")?;
                if !step.precedence.is_none()
                    || step.associativity.is_some()
                    || step.reserved_word_set_id != ReservedWordSetId::default()
                {
                    write!(f, " (")?;
                    if step.precedence.is_none() {
                        write!(f, " {}", step.precedence)?;
                    }
                    if let Some(associativity) = step.associativity {
                        write!(f, " {associativity:?}")?;
                    }
                    if step.reserved_word_set_id != ReservedWordSetId::default() {
                        write!(f, "reserved: {}", step.reserved_word_set_id)?;
                    }
                    write!(f, " )")?;
                }
            }

            write!(f, " ")?;
            if step.symbol.is_terminal() {
                if let Some(variable) = self.2.variables.get(step.symbol.index) {
                    write!(f, "{}", variable.name)?;
                } else {
                    write!(f, "terminal-{}", step.symbol.index)?;
                }
            } else if step.symbol.is_external() {
                write!(f, "{}", self.1.external_tokens[step.symbol.index].name)?;
            } else {
                write!(f, "{}", self.1.variables[step.symbol.index].name)?;
            }

            if let Some(alias) = &step.alias {
                write!(f, "@{}", alias.value)?;
            }
        }

        if self.0.is_done() {
            write!(f, " •")?;
            if let Some(step) = self.0.production.steps.last() {
                if let Some(associativity) = step.associativity {
                    if step.precedence.is_none() {
                        write!(f, " ({associativity:?})")?;
                    } else {
                        write!(f, " ({} {associativity:?})", step.precedence)?;
                    }
                } else if !step.precedence.is_none() {
                    write!(f, " ({})", step.precedence)?;
                }
            }
        }

        Ok(())
    }
}

const fn escape_invisible(c: char) -> Option<&'static str> {
    Some(match c {
        '\n' => "\\n",
        '\r' => "\\r",
        '\t' => "\\t",
        '\0' => "\\0",
        '\\' => "\\\\",
        '\x0b' => "\\v",
        '\x0c' => "\\f",
        _ => return None,
    })
}

fn display_variable_name(source: &str) -> String {
    source
        .chars()
        .fold(String::with_capacity(source.len()), |mut acc, c| {
            if let Some(esc) = escape_invisible(c) {
                acc.push_str(esc);
            } else {
                acc.push(c);
            }
            acc
        })
}

impl fmt::Display for TokenSetDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "[")?;
        for (i, symbol) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }

            if symbol.is_terminal() {
                if let Some(variable) = self.2.variables.get(symbol.index) {
                    write!(f, "{}", display_variable_name(&variable.name))?;
                } else {
                    write!(f, "terminal-{}", symbol.index)?;
                }
            } else if symbol.is_external() {
                write!(f, "{}", self.1.external_tokens[symbol.index].name)?;
            } else {
                write!(f, "{}", self.1.variables[symbol.index].name)?;
            }
        }
        write!(f, "]")?;
        Ok(())
    }
}

impl fmt::Display for ParseItemSetDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        for entry in &self.0.entries {
            write!(
                f,
                "{}\t{}",
                ParseItemDisplay(&entry.item, self.1, self.2),
                TokenSetDisplay(&entry.lookaheads, self.1, self.2),
            )?;
            if entry.following_reserved_word_set != ReservedWordSetId::default() {
                write!(
                    f,
                    "\treserved word set: {}",
                    entry.following_reserved_word_set
                )?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl Hash for ParseItem<'_> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write_u32(self.variable_index);
        hasher.write_u32(self.step_index);
        hasher.write_i32(self.production.dynamic_precedence);
        hasher.write_usize(self.production.steps.len());
        hasher.write_i32(i32::from(self.has_preceding_inherited_fields));
        self.precedence().hash(hasher);
        self.associativity().hash(hasher);

        // The already-matched children don't play any role in the parse state for
        // this item, unless any of the following are true:
        //   * the children have fields
        //   * the children have aliases
        //   * the children are hidden and represent rules that have fields.
        // See the docs for `has_preceding_inherited_fields`.
        for step in &self.production.steps[0..self.step_index as usize] {
            step.alias.hash(hasher);
            step.field_name.hash(hasher);
            if self.has_preceding_inherited_fields {
                step.symbol.hash(hasher);
            }
        }
        for step in &self.production.steps[self.step_index as usize..] {
            step.hash(hasher);
        }
    }
}

impl PartialEq for ParseItem<'_> {
    fn eq(&self, other: &Self) -> bool {
        if self.variable_index != other.variable_index
            || self.step_index != other.step_index
            || self.production.dynamic_precedence != other.production.dynamic_precedence
            || self.production.steps.len() != other.production.steps.len()
            || self.precedence() != other.precedence()
            || self.associativity() != other.associativity()
            || self.has_preceding_inherited_fields != other.has_preceding_inherited_fields
        {
            return false;
        }

        for (i, step) in self.production.steps.iter().enumerate() {
            // See the previous comment (in the `Hash::hash` impl) regarding comparisons
            // of parse items' already-completed steps.
            if i < self.step_index as usize {
                if step.alias != other.production.steps[i].alias {
                    return false;
                }
                if step.field_name != other.production.steps[i].field_name {
                    return false;
                }
                if self.has_preceding_inherited_fields
                    && step.symbol != other.production.steps[i].symbol
                {
                    return false;
                }
            } else if *step != other.production.steps[i] {
                return false;
            }
        }

        true
    }
}

impl Ord for ParseItem<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.step_index
            .cmp(&other.step_index)
            .then_with(|| self.variable_index.cmp(&other.variable_index))
            .then_with(|| {
                self.production
                    .dynamic_precedence
                    .cmp(&other.production.dynamic_precedence)
            })
            .then_with(|| {
                self.production
                    .steps
                    .len()
                    .cmp(&other.production.steps.len())
            })
            .then_with(|| self.precedence().cmp(other.precedence()))
            .then_with(|| self.associativity().cmp(&other.associativity()))
            .then_with(|| {
                for (i, step) in self.production.steps.iter().enumerate() {
                    // See the previous comment (in the `Hash::hash` impl) regarding comparisons
                    // of parse items' already-completed steps.
                    let o = if i < self.step_index as usize {
                        step.alias
                            .cmp(&other.production.steps[i].alias)
                            .then_with(|| {
                                step.field_name.cmp(&other.production.steps[i].field_name)
                            })
                    } else {
                        step.cmp(&other.production.steps[i])
                    };
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                Ordering::Equal
            })
    }
}

impl PartialOrd for ParseItem<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for ParseItem<'_> {}

impl Hash for ParseItemSet<'_> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write_usize(self.entries.len());
        for entry in &self.entries {
            entry.item.hash(hasher);
            entry.lookaheads.hash(hasher);
            entry.following_reserved_word_set.hash(hasher);
        }
    }
}

impl Hash for ParseItemSetCore<'_> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        hasher.write_usize(self.entries.len());
        for item in &self.entries {
            item.hash(hasher);
        }
    }
}
