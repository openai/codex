//! Inlining of nonterminals

use crate::grammar::repr::*;
use crate::normalize::NormResult;

mod graph;

#[cfg(test)]
mod test;

pub fn inline(mut grammar: Grammar) -> NormResult<Grammar> {
    let order = graph::inline_order(&grammar)?;
    for nt in order {
        inline_nt(&mut grammar, &nt);
    }
    Ok(grammar)
}

fn inline_nt(grammar: &mut Grammar, inline_nt: &NonterminalString) {
    let inline_productions = grammar.productions_for(inline_nt).to_vec();
    for data in grammar.nonterminals.values_mut() {
        let mut new_productions = vec![];
        let mut new_action_fn_defns = vec![];

        for into_production in &data.productions {
            if !into_production
                .symbols
                .contains(&Symbol::Nonterminal(inline_nt.clone()))
            {
                new_productions.push(into_production.clone());
                continue;
            }

            let mut inliner = Inliner {
                action_fn_defns: &grammar.action_fn_defns,
                inline_nonterminal: inline_nt.clone(),
                into_production,
                inline_fallible: 0,
                inline_productions: &inline_productions,
                new_symbols: vec![],
                new_productions: &mut new_productions,
                new_action_fn_defns: &mut new_action_fn_defns,
            };

            inliner.inline(&into_production.symbols);
        }

        data.productions = new_productions;
        grammar.action_fn_defns.extend(new_action_fn_defns);
    }
}

struct Inliner<'a> {
    /// Action fn defns
    action_fn_defns: &'a [ActionFnDefn],

    /// The nonterminal `A` being inlined
    inline_nonterminal: NonterminalString,

    /// The full set of productions `A = B C D | E F G` for the
    /// nonterminal `A` being inlined
    inline_productions: &'a [Production],

    /// Number of actions that we have inlined for `A` so far which
    /// have been fallible. IOW, if we are inlining `A` into `X = Y A
    /// A Z`, and in the first instance of `A` we used a fallible
    /// action, but the second we used an infallible one, count would
    /// be 1.
    inline_fallible: u32,

    /// The `X = Y A Z` being inlined into
    into_production: &'a Production,

    /// The list of symbols we building up for the new production.
    /// For example, this would (eventually) contain `Y B C D Z`,
    /// given our running example.
    new_symbols: Vec<InlinedSymbol>,

    /// The output vector of all productions for `X` that we have created
    new_productions: &'a mut Vec<Production>,

    /// Vector of all action fn defns from the grammar.
    new_action_fn_defns: &'a mut Vec<ActionFnDefn>,
}

impl<'a> Inliner<'a> {
    fn inline(&mut self, into_symbols: &[Symbol]) {
        if into_symbols.is_empty() {
            // create an action fn for the result of inlining
            let into_action = self.into_production.action;
            let into_fallible = self.action_fn_defns[into_action.index()].fallible;
            let into_ret_type = self.action_fn_defns[into_action.index()].ret_type.clone();
            let inline_fallible = self.inline_fallible != 0;
            let index = self.action_fn_defns.len() + self.new_action_fn_defns.len();
            let action_fn = ActionFn::new(index);
            let inline_defn = InlineActionFnDefn {
                action: into_action,
                symbols: self.new_symbols.clone(),
            };
            self.new_action_fn_defns.push(ActionFnDefn {
                fallible: into_fallible || inline_fallible,
                ret_type: into_ret_type,
                kind: ActionFnDefnKind::Inline(inline_defn),
            });
            let prod_symbols: Vec<Symbol> = self
                .new_symbols
                .iter()
                .flat_map(|sym| match *sym {
                    InlinedSymbol::Original(ref s) => vec![s.clone()],
                    InlinedSymbol::Inlined(_, ref s) => s.clone(),
                })
                .collect();
            self.new_productions.push(Production {
                nonterminal: self.into_production.nonterminal.clone(),
                span: self.into_production.span,
                symbols: prod_symbols,
                action: action_fn,
            });
        } else {
            let next_symbol = &into_symbols[0];
            match *next_symbol {
                Symbol::Nonterminal(ref n) if *n == self.inline_nonterminal => {
                    // Replace the current symbol with each of the
                    // `inline_productions` in turn.
                    for inline_production in self.inline_productions {
                        // If this production is fallible, increment
                        // count of fallible actions.
                        let inline_action = inline_production.action;
                        let fallible = self.action_fn_defns[inline_action.index()].fallible;
                        self.inline_fallible += fallible as u32;

                        // Push the symbols of the production inline.
                        self.new_symbols.push(InlinedSymbol::Inlined(
                            inline_production.action,
                            inline_production.symbols.clone(),
                        ));

                        // Inline remaining symbols:
                        self.inline(&into_symbols[1..]);

                        // Reset state after we have inlined remaining symbols:
                        self.new_symbols.pop();
                        self.inline_fallible -= fallible as u32;
                    }
                }
                _ => {
                    self.new_symbols
                        .push(InlinedSymbol::Original(next_symbol.clone()));
                    self.inline(&into_symbols[1..]);
                    self.new_symbols.pop();
                }
            }
        }
    }
}
