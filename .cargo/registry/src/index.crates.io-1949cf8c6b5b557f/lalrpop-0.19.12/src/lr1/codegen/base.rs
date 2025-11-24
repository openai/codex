//! Base helper routines for a code generator.

use crate::collections::Set;
use crate::grammar::free_variables::FreeVariables;
use crate::grammar::repr::*;
use crate::lr1::core::*;
use crate::rust::RustWrite;
use crate::util::Sep;
use std::io::{self, Write};

/// Base struct for various kinds of code generator. The flavor of
/// code generator is customized by supplying distinct types for `C`
/// (e.g., `self::ascent::RecursiveAscent`).
pub struct CodeGenerator<'codegen, 'grammar: 'codegen, W: Write + 'codegen, C> {
    /// the complete grammar
    pub grammar: &'grammar Grammar,

    /// some suitable prefix to separate our identifiers from the user's
    pub prefix: &'grammar str,

    /// types from the grammar
    pub types: &'grammar Types,

    /// the start symbol S the user specified
    pub user_start_symbol: NonterminalString,

    /// the synthetic start symbol S' that we specified
    pub start_symbol: NonterminalString,

    /// the vector of states
    pub states: &'codegen [LR1State<'grammar>],

    /// where we write output
    pub out: &'codegen mut RustWrite<W>,

    /// where to find the action routines (typically `super`)
    pub action_module: String,

    /// custom fields for the specific kind of codegenerator
    /// (recursive ascent, table-driven, etc)
    pub custom: C,

    pub repeatable: bool,
}

impl<'codegen, 'grammar, W: Write, C> CodeGenerator<'codegen, 'grammar, W, C> {
    pub fn new(
        grammar: &'grammar Grammar,
        user_start_symbol: NonterminalString,
        start_symbol: NonterminalString,
        states: &'codegen [LR1State<'grammar>],
        out: &'codegen mut RustWrite<W>,
        repeatable: bool,
        action_module: &str,
        custom: C,
    ) -> Self {
        CodeGenerator {
            grammar,
            prefix: &grammar.prefix,
            types: &grammar.types,
            states,
            user_start_symbol,
            start_symbol,
            out,
            custom,
            repeatable,
            action_module: action_module.to_string(),
        }
    }

    /// We often create meta types that pull together a bunch of
    /// user-given types -- basically describing (e.g.) the full set
    /// of return values from any nonterminal (and, in some cases,
    /// terminals). These types need to carry generic parameters from
    /// the grammar, since the nonterminals may include generic
    /// parameters -- but we don't want them to carry *all* the
    /// generic parameters, since that can be unnecessarily
    /// restrictive.
    ///
    /// In particular, consider something like this:
    ///
    /// ```notrust
    /// grammar<'a>(buffer: &'a mut Vec<u32>);
    /// ```
    ///
    /// Here, we likely do not want the `'a` in the type of `buffer` to appear
    /// in the nonterminal result. That's because, if it did, then the
    /// action functions will have a signature like:
    ///
    /// ```ignore
    /// fn foo<'a, T>(x: &'a mut Vec<T>) -> Result<'a> { ... }
    /// ```
    ///
    /// In that case, we would only be able to call one action fn and
    /// will in fact get borrowck errors, because Rust would think we
    /// were potentially returning this `&'a mut Vec<T>`.
    ///
    /// Therefore, we take the full list of type parameters and we
    /// filter them down to those that appear in the types that we
    /// need to include (those that appear in the `tys` parameter).
    ///
    /// In some cases, we need to include a few more than just that
    /// obviously appear textually: for example, if we have `T::Foo`,
    /// and we see a where-clause `T: Bar<'a>`, then we need to
    /// include both `T` and `'a`, since that bound may be important
    /// for resolving `T::Foo` (in other words, `T::Foo` may expand to
    /// `<T as Bar<'a>>::Foo`).
    pub fn filter_type_parameters_and_where_clauses(
        grammar: &Grammar,
        tys: impl IntoIterator<Item = TypeRepr>,
    ) -> (Vec<TypeParameter>, Vec<WhereClause>) {
        let referenced_ty_params: Set<_> = tys
            .into_iter()
            .flat_map(|t| t.free_variables(&grammar.type_parameters))
            .collect();

        let filtered_type_params: Vec<_> = grammar
            .type_parameters
            .iter()
            .filter(|t| referenced_ty_params.contains(t))
            .cloned()
            .collect();

        // If `T` is referenced in the types we need to keep, then
        // include any bounds like `T: Foo`. This may be needed for
        // the well-formedness conditions on `T` (e.g., maybe we have
        // `T: Hash` and a `HashSet<T>` or something) but it may also
        // be needed because of `T::Foo`-like types.
        //
        // Do not however include a bound like `T: 'a` unless both `T`
        // **and** `'a` are referenced -- same with bounds like `T:
        // Foo<U>`. If those were needed, then `'a` or `U` would also
        // have to appear in the types.
        debug!("filtered_type_params = {:?}", filtered_type_params);
        let filtered_where_clauses: Vec<_> = grammar
            .where_clauses
            .iter()
            .filter(|wc| {
                debug!(
                    "wc = {:?} free_variables = {:?}",
                    wc,
                    wc.free_variables(&grammar.type_parameters)
                );
                wc.free_variables(&grammar.type_parameters)
                    .iter()
                    .all(|p| referenced_ty_params.contains(p))
            })
            .cloned()
            .collect();
        debug!("filtered_where_clauses = {:?}", filtered_where_clauses);

        (filtered_type_params, filtered_where_clauses)
    }

    pub fn write_parse_mod<F>(&mut self, body: F) -> io::Result<()>
    where
        F: FnOnce(&mut Self) -> io::Result<()>,
    {
        rust!(self.out, "");
        rust!(self.out, "#[cfg_attr(rustfmt, rustfmt_skip)]");
        rust!(self.out, "mod {}parse{} {{", self.prefix, self.start_symbol);

        // these stylistic lints are annoying for the generated code,
        // which doesn't follow conventions:
        rust!(
            self.out,
            "#![allow(non_snake_case, non_camel_case_types, unused_mut, unused_variables, \
             unused_imports, unused_parens, clippy::all)]"
        );
        rust!(self.out, "");

        self.write_uses()?;

        body(self)?;

        rust!(self.out, "}}");
        Ok(())
    }

    pub fn write_uses(&mut self) -> io::Result<()> {
        self.out
            .write_uses(&format!("{}::", self.action_module), self.grammar)?;

        if self.grammar.intern_token.is_some() {
            rust!(
                self.out,
                "use self::{}lalrpop_util::lexer::Token;",
                self.prefix
            );
        } else {
            rust!(
                self.out,
                "use {}::{}ToTriple;",
                self.action_module,
                self.prefix
            );
        }

        Ok(())
    }

    pub fn start_parser_fn(&mut self) -> io::Result<()> {
        let parse_error_type = self.types.parse_error_type();

        let (type_parameters, parameters, mut where_clauses);

        let intern_token = self.grammar.intern_token.is_some();
        if intern_token {
            // if we are generating the tokenizer, we just need the
            // input, and that has already been added as one of the
            // user parameters
            type_parameters = vec![];
            parameters = vec![];
            where_clauses = vec![];
        } else {
            // otherwise, we need an iterator of type `TOKENS`
            let mut user_type_parameters = String::new();
            for type_parameter in &self.grammar.type_parameters {
                user_type_parameters.push_str(&format!("{}, ", type_parameter));
            }
            type_parameters = vec![
                format!(
                    "{}TOKEN: {}ToTriple<{}>",
                    self.prefix, self.prefix, user_type_parameters,
                ),
                format!(
                    "{}TOKENS: IntoIterator<Item={}TOKEN>",
                    self.prefix, self.prefix
                ),
            ];
            parameters = vec![format!("{}tokens0: {}TOKENS", self.prefix, self.prefix)];
            where_clauses = vec![];

            if self.repeatable {
                where_clauses.push(format!("{}TOKENS: Clone", self.prefix));
            }
        }

        rust!(
            self.out,
            "{}struct {}Parser {{",
            self.grammar.nonterminals[&self.start_symbol].visibility,
            self.user_start_symbol
        );
        if intern_token {
            rust!(
                self.out,
                "builder: {}lalrpop_util::lexer::MatcherBuilder,",
                self.prefix,
            );
        }
        rust!(self.out, "_priv: (),");
        rust!(self.out, "}}");
        rust!(self.out, "");

        rust!(self.out, "impl {}Parser {{", self.user_start_symbol);
        rust!(
            self.out,
            "{}fn new() -> {}Parser {{",
            self.grammar.nonterminals[&self.start_symbol].visibility,
            self.user_start_symbol
        );
        if intern_token {
            rust!(
                self.out,
                "let {0}builder = {1}::{0}intern_token::new_builder();",
                self.prefix,
                self.action_module
            );
        }
        rust!(self.out, "{}Parser {{", self.user_start_symbol);
        if intern_token {
            rust!(self.out, "builder: {}builder,", self.prefix);
        }
        rust!(self.out, "_priv: (),");
        rust!(self.out, "}}"); // Parser
        rust!(self.out, "}}"); // new()
        rust!(self.out, "");

        rust!(self.out, "#[allow(dead_code)]");
        self.out
            .fn_header(
                &self.grammar.nonterminals[&self.start_symbol].visibility,
                "parse".to_owned(),
            )
            .with_parameters(Some("&self".to_owned()))
            .with_grammar(self.grammar)
            .with_type_parameters(type_parameters)
            .with_parameters(parameters)
            .with_return_type(format!(
                "Result<{}, {}>",
                self.types.nonterminal_type(&self.start_symbol),
                parse_error_type
            ))
            .with_where_clauses(where_clauses)
            .emit()?;
        rust!(self.out, "{{");

        Ok(())
    }

    pub fn define_tokens(&mut self) -> io::Result<()> {
        if self.grammar.intern_token.is_some() {
            // if we are generating the tokenizer, create a matcher as our input iterator
            rust!(
                self.out,
                "let mut {}tokens = self.builder.matcher(input);",
                self.prefix
            );
        } else {
            // otherwise, convert one from the `IntoIterator`
            // supplied, using the `ToTriple` trait which inserts
            // errors/locations etc if none are given
            let clone_call = if self.repeatable { ".clone()" } else { "" };
            rust!(
                self.out,
                "let {}tokens = {}tokens0{}.into_iter();",
                self.prefix,
                self.prefix,
                clone_call
            );

            rust!(
                self.out,
                "let mut {}tokens = {}tokens.map(|t| {}ToTriple::to_triple(t));",
                self.prefix,
                self.prefix,
                self.prefix
            );
        }

        Ok(())
    }

    pub fn end_parser_fn(&mut self) -> io::Result<()> {
        rust!(self.out, "}}"); // fn
        rust!(self.out, "}}"); // impl
        Ok(())
    }

    /// Returns phantom data type that captures the user-declared type
    /// parameters in a phantom-data. This helps with ensuring that
    /// all type parameters are constrained, even if they are not
    /// used.
    pub fn phantom_data_type(&self) -> String {
        let phantom_bits: Vec<_> = self
            .grammar
            .type_parameters
            .iter()
            .map(|tp| match *tp {
                TypeParameter::Lifetime(ref l) => format!("&{} ()", l),

                TypeParameter::Id(ref id) => id.to_string(),
            })
            .collect();
        format!("core::marker::PhantomData<({})>", Sep(", ", &phantom_bits),)
    }

    /// Returns expression that captures the user-declared type
    /// parameters in a phantom-data. This helps with ensuring that
    /// all type parameters are constrained, even if they are not
    /// used.
    pub fn phantom_data_expr(&self) -> String {
        let phantom_bits: Vec<_> = self
            .grammar
            .type_parameters
            .iter()
            .map(|tp| match *tp {
                TypeParameter::Lifetime(_) => "&()".to_string(),
                TypeParameter::Id(ref id) => id.to_string(),
            })
            .collect();
        format!(
            "core::marker::PhantomData::<({})>",
            Sep(", ", &phantom_bits),
        )
    }
}
