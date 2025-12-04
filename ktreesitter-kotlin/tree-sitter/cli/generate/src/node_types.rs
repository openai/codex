use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use serde::Serialize;
use thiserror::Error;

use super::{
    grammars::{LexicalGrammar, SyntaxGrammar, VariableType},
    rules::{Alias, AliasMap, Symbol, SymbolType},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChildType {
    Normal(Symbol),
    Aliased(Alias),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldInfo {
    pub quantity: ChildQuantity,
    pub types: Vec<ChildType>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VariableInfo {
    pub fields: HashMap<String, FieldInfo>,
    pub children: FieldInfo,
    pub children_without_fields: FieldInfo,
    pub has_multi_step_production: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct NodeInfoJSON {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    root: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    extra: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<BTreeMap<String, FieldInfoJSON>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<FieldInfoJSON>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtypes: Option<Vec<NodeTypeJSON>>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeTypeJSON {
    #[serde(rename = "type")]
    kind: String,
    named: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct FieldInfoJSON {
    multiple: bool,
    required: bool,
    types: Vec<NodeTypeJSON>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildQuantity {
    exists: bool,
    required: bool,
    multiple: bool,
}

impl Default for FieldInfoJSON {
    fn default() -> Self {
        Self {
            multiple: false,
            required: true,
            types: Vec::new(),
        }
    }
}

impl Default for ChildQuantity {
    fn default() -> Self {
        Self::one()
    }
}

impl ChildQuantity {
    #[must_use]
    const fn zero() -> Self {
        Self {
            exists: false,
            required: false,
            multiple: false,
        }
    }

    #[must_use]
    const fn one() -> Self {
        Self {
            exists: true,
            required: true,
            multiple: false,
        }
    }

    fn append(&mut self, other: Self) {
        if other.exists {
            if self.exists || other.multiple {
                self.multiple = true;
            }
            if other.required {
                self.required = true;
            }
            self.exists = true;
        }
    }

    fn union(&mut self, other: Self) -> bool {
        let mut result = false;
        if !self.exists && other.exists {
            result = true;
            self.exists = true;
        }
        if self.required && !other.required {
            result = true;
            self.required = false;
        }
        if !self.multiple && other.multiple {
            result = true;
            self.multiple = true;
        }
        result
    }
}

pub type VariableInfoResult<T> = Result<T, VariableInfoError>;

#[derive(Debug, Error, Serialize)]
pub enum VariableInfoError {
    #[error("Grammar error: Supertype symbols must always have a single visible child, but `{0}` can have multiple")]
    InvalidSupertype(String),
}

/// Compute a summary of the public-facing structure of each variable in the
/// grammar. Each variable in the grammar corresponds to a distinct public-facing
/// node type.
///
/// The information collected about each node type `N` is:
/// 1. `child_types` - The types of visible children that can appear within `N`.
/// 2. `fields` - The fields that `N` can have. Data regarding each field:
///    * `types` - The types of visible children the field can contain.
///    * `optional` - Do `N` nodes always have this field?
///    * `multiple` - Can `N` nodes have multiple children for this field?
/// 3. `children_without_fields` - The *other* named children of `N` that are not associated with
///    fields. Data regarding these children:
///    * `types` - The types of named children with no field.
///    * `optional` - Do `N` nodes always have at least one named child with no field?
///    * `multiple` - Can `N` nodes have multiple named children with no field?
///
/// Each summary must account for some indirect factors:
/// 1. hidden nodes. When a parent node `N` has a hidden child `C`, the visible children of `C`
///    *appear* to be direct children of `N`.
/// 2. aliases. If a parent node type `M` is aliased as some other type `N`, then nodes which
///    *appear* to have type `N` may have internal structure based on `M`.
pub fn get_variable_info(
    syntax_grammar: &SyntaxGrammar,
    lexical_grammar: &LexicalGrammar,
    default_aliases: &AliasMap,
) -> VariableInfoResult<Vec<VariableInfo>> {
    let child_type_is_visible = |t: &ChildType| {
        variable_type_for_child_type(t, syntax_grammar, lexical_grammar) >= VariableType::Anonymous
    };

    let child_type_is_named = |t: &ChildType| {
        variable_type_for_child_type(t, syntax_grammar, lexical_grammar) == VariableType::Named
    };

    // Each variable's summary can depend on the summaries of other hidden variables,
    // and variables can have mutually recursive structure. So we compute the summaries
    // iteratively, in a loop that terminates only when no more changes are possible.
    let mut did_change = true;
    let mut all_initialized = false;
    let mut result = vec![VariableInfo::default(); syntax_grammar.variables.len()];
    while did_change {
        did_change = false;

        for (i, variable) in syntax_grammar.variables.iter().enumerate() {
            let mut variable_info = result[i].clone();

            // Examine each of the variable's productions. The variable's child types can be
            // immediately combined across all productions, but the child quantities must be
            // recorded separately for each production.
            for production in &variable.productions {
                let mut production_field_quantities = HashMap::new();
                let mut production_children_quantity = ChildQuantity::zero();
                let mut production_children_without_fields_quantity = ChildQuantity::zero();
                let mut production_has_uninitialized_invisible_children = false;

                if production.steps.len() > 1 {
                    variable_info.has_multi_step_production = true;
                }

                for step in &production.steps {
                    let child_symbol = step.symbol;
                    let child_type = if let Some(alias) = &step.alias {
                        ChildType::Aliased(alias.clone())
                    } else if let Some(alias) = default_aliases.get(&step.symbol) {
                        ChildType::Aliased(alias.clone())
                    } else {
                        ChildType::Normal(child_symbol)
                    };

                    let child_is_hidden = !child_type_is_visible(&child_type)
                        && !syntax_grammar.supertype_symbols.contains(&child_symbol);

                    // Maintain the set of all child types for this variable, and the quantity of
                    // visible children in this production.
                    did_change |=
                        extend_sorted(&mut variable_info.children.types, Some(&child_type));
                    if !child_is_hidden {
                        production_children_quantity.append(ChildQuantity::one());
                    }

                    // Maintain the set of child types associated with each field, and the quantity
                    // of children associated with each field in this production.
                    if let Some(field_name) = &step.field_name {
                        let field_info = variable_info
                            .fields
                            .entry(field_name.clone())
                            .or_insert_with(FieldInfo::default);
                        did_change |= extend_sorted(&mut field_info.types, Some(&child_type));

                        let production_field_quantity = production_field_quantities
                            .entry(field_name)
                            .or_insert_with(ChildQuantity::zero);

                        // Inherit the types and quantities of hidden children associated with
                        // fields.
                        if child_is_hidden && child_symbol.is_non_terminal() {
                            let child_variable_info = &result[child_symbol.index];
                            did_change |= extend_sorted(
                                &mut field_info.types,
                                &child_variable_info.children.types,
                            );
                            production_field_quantity.append(child_variable_info.children.quantity);
                        } else {
                            production_field_quantity.append(ChildQuantity::one());
                        }
                    }
                    // Maintain the set of named children without fields within this variable.
                    else if child_type_is_named(&child_type) {
                        production_children_without_fields_quantity.append(ChildQuantity::one());
                        did_change |= extend_sorted(
                            &mut variable_info.children_without_fields.types,
                            Some(&child_type),
                        );
                    }

                    // Inherit all child information from hidden children.
                    if child_is_hidden && child_symbol.is_non_terminal() {
                        let child_variable_info = &result[child_symbol.index];

                        // If a hidden child can have multiple children, then its parent node can
                        // appear to have multiple children.
                        if child_variable_info.has_multi_step_production {
                            variable_info.has_multi_step_production = true;
                        }

                        // If a hidden child has fields, then the parent node can appear to have
                        // those same fields.
                        for (field_name, child_field_info) in &child_variable_info.fields {
                            production_field_quantities
                                .entry(field_name)
                                .or_insert_with(ChildQuantity::zero)
                                .append(child_field_info.quantity);
                            did_change |= extend_sorted(
                                &mut variable_info
                                    .fields
                                    .entry(field_name.clone())
                                    .or_insert_with(FieldInfo::default)
                                    .types,
                                &child_field_info.types,
                            );
                        }

                        // If a hidden child has children, then the parent node can appear to have
                        // those same children.
                        production_children_quantity.append(child_variable_info.children.quantity);
                        did_change |= extend_sorted(
                            &mut variable_info.children.types,
                            &child_variable_info.children.types,
                        );

                        // If a hidden child can have named children without fields, then the parent
                        // node can appear to have those same children.
                        if step.field_name.is_none() {
                            let grandchildren_info = &child_variable_info.children_without_fields;
                            if !grandchildren_info.types.is_empty() {
                                production_children_without_fields_quantity
                                    .append(child_variable_info.children_without_fields.quantity);
                                did_change |= extend_sorted(
                                    &mut variable_info.children_without_fields.types,
                                    &child_variable_info.children_without_fields.types,
                                );
                            }
                        }
                    }

                    // Note whether or not this production contains children whose summaries
                    // have not yet been computed.
                    if child_symbol.index >= i && !all_initialized {
                        production_has_uninitialized_invisible_children = true;
                    }
                }

                // If this production's children all have had their summaries initialized,
                // then expand the quantity information with all of the possibilities introduced
                // by this production.
                if !production_has_uninitialized_invisible_children {
                    did_change |= variable_info
                        .children
                        .quantity
                        .union(production_children_quantity);

                    did_change |= variable_info
                        .children_without_fields
                        .quantity
                        .union(production_children_without_fields_quantity);

                    for (field_name, info) in &mut variable_info.fields {
                        did_change |= info.quantity.union(
                            production_field_quantities
                                .get(field_name)
                                .copied()
                                .unwrap_or_else(ChildQuantity::zero),
                        );
                    }
                }
            }

            result[i] = variable_info;
        }

        all_initialized = true;
    }

    for supertype_symbol in &syntax_grammar.supertype_symbols {
        if result[supertype_symbol.index].has_multi_step_production {
            let variable = &syntax_grammar.variables[supertype_symbol.index];
            Err(VariableInfoError::InvalidSupertype(variable.name.clone()))?;
        }
    }

    // Update all of the node type lists to eliminate hidden nodes.
    for supertype_symbol in &syntax_grammar.supertype_symbols {
        result[supertype_symbol.index]
            .children
            .types
            .retain(child_type_is_visible);
    }
    for variable_info in &mut result {
        for field_info in variable_info.fields.values_mut() {
            field_info.types.retain(child_type_is_visible);
        }
        variable_info.fields.retain(|_, v| !v.types.is_empty());
        variable_info
            .children_without_fields
            .types
            .retain(child_type_is_visible);
    }

    Ok(result)
}

fn get_aliases_by_symbol(
    syntax_grammar: &SyntaxGrammar,
    default_aliases: &AliasMap,
) -> HashMap<Symbol, HashSet<Option<Alias>>> {
    let mut aliases_by_symbol = HashMap::new();
    for (symbol, alias) in default_aliases {
        aliases_by_symbol.insert(*symbol, {
            let mut aliases = HashSet::new();
            aliases.insert(Some(alias.clone()));
            aliases
        });
    }
    for extra_symbol in &syntax_grammar.extra_symbols {
        if !default_aliases.contains_key(extra_symbol) {
            aliases_by_symbol
                .entry(*extra_symbol)
                .or_insert_with(HashSet::new)
                .insert(None);
        }
    }
    for variable in &syntax_grammar.variables {
        for production in &variable.productions {
            for step in &production.steps {
                aliases_by_symbol
                    .entry(step.symbol)
                    .or_insert_with(HashSet::new)
                    .insert(
                        step.alias
                            .as_ref()
                            .or_else(|| default_aliases.get(&step.symbol))
                            .cloned(),
                    );
            }
        }
    }
    aliases_by_symbol.insert(
        Symbol::non_terminal(0),
        std::iter::once(&None).cloned().collect(),
    );
    aliases_by_symbol
}

pub fn get_supertype_symbol_map(
    syntax_grammar: &SyntaxGrammar,
    default_aliases: &AliasMap,
    variable_info: &[VariableInfo],
) -> BTreeMap<Symbol, Vec<ChildType>> {
    let aliases_by_symbol = get_aliases_by_symbol(syntax_grammar, default_aliases);
    let mut supertype_symbol_map = BTreeMap::new();

    let mut symbols_by_alias = HashMap::new();
    for (symbol, aliases) in &aliases_by_symbol {
        for alias in aliases.iter().flatten() {
            symbols_by_alias
                .entry(alias)
                .or_insert_with(Vec::new)
                .push(*symbol);
        }
    }

    for (i, info) in variable_info.iter().enumerate() {
        let symbol = Symbol::non_terminal(i);
        if syntax_grammar.supertype_symbols.contains(&symbol) {
            let subtypes = info.children.types.clone();
            supertype_symbol_map.insert(symbol, subtypes);
        }
    }
    supertype_symbol_map
}

pub type SuperTypeCycleResult<T> = Result<T, SuperTypeCycleError>;

#[derive(Debug, Error, Serialize)]
pub struct SuperTypeCycleError {
    items: Vec<String>,
}

impl std::fmt::Display for SuperTypeCycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dependency cycle detected in node types:")?;
        for (i, item) in self.items.iter().enumerate() {
            write!(f, " {item}")?;
            if i < self.items.len() - 1 {
                write!(f, ",")?;
            }
        }

        Ok(())
    }
}

pub fn generate_node_types_json(
    syntax_grammar: &SyntaxGrammar,
    lexical_grammar: &LexicalGrammar,
    default_aliases: &AliasMap,
    variable_info: &[VariableInfo],
) -> SuperTypeCycleResult<Vec<NodeInfoJSON>> {
    let mut node_types_json = BTreeMap::new();

    let child_type_to_node_type = |child_type: &ChildType| match child_type {
        ChildType::Aliased(alias) => NodeTypeJSON {
            kind: alias.value.clone(),
            named: alias.is_named,
        },
        ChildType::Normal(symbol) => {
            if let Some(alias) = default_aliases.get(symbol) {
                NodeTypeJSON {
                    kind: alias.value.clone(),
                    named: alias.is_named,
                }
            } else {
                match symbol.kind {
                    SymbolType::NonTerminal => {
                        let variable = &syntax_grammar.variables[symbol.index];
                        NodeTypeJSON {
                            kind: variable.name.clone(),
                            named: variable.kind != VariableType::Anonymous,
                        }
                    }
                    SymbolType::Terminal => {
                        let variable = &lexical_grammar.variables[symbol.index];
                        NodeTypeJSON {
                            kind: variable.name.clone(),
                            named: variable.kind != VariableType::Anonymous,
                        }
                    }
                    SymbolType::External => {
                        let variable = &syntax_grammar.external_tokens[symbol.index];
                        NodeTypeJSON {
                            kind: variable.name.clone(),
                            named: variable.kind != VariableType::Anonymous,
                        }
                    }
                    _ => panic!("Unexpected symbol type"),
                }
            }
        }
    };

    let populate_field_info_json = |json: &mut FieldInfoJSON, info: &FieldInfo| {
        if info.types.is_empty() {
            json.required = false;
        } else {
            json.multiple |= info.quantity.multiple;
            json.required &= info.quantity.required;
            json.types
                .extend(info.types.iter().map(child_type_to_node_type));
            json.types.sort_unstable();
            json.types.dedup();
        }
    };

    let aliases_by_symbol = get_aliases_by_symbol(syntax_grammar, default_aliases);

    let empty = HashSet::new();
    let extra_names = syntax_grammar
        .extra_symbols
        .iter()
        .flat_map(|symbol| {
            aliases_by_symbol
                .get(symbol)
                .unwrap_or(&empty)
                .iter()
                .map(|alias| {
                    alias.as_ref().map_or(
                        match symbol.kind {
                            SymbolType::NonTerminal => &syntax_grammar.variables[symbol.index].name,
                            SymbolType::Terminal => &lexical_grammar.variables[symbol.index].name,
                            SymbolType::External => {
                                &syntax_grammar.external_tokens[symbol.index].name
                            }
                            _ => unreachable!(),
                        },
                        |alias| &alias.value,
                    )
                })
        })
        .collect::<HashSet<_>>();

    let mut subtype_map = Vec::new();
    for (i, info) in variable_info.iter().enumerate() {
        let symbol = Symbol::non_terminal(i);
        let variable = &syntax_grammar.variables[i];
        if syntax_grammar.supertype_symbols.contains(&symbol) {
            let node_type_json =
                node_types_json
                    .entry(variable.name.clone())
                    .or_insert_with(|| NodeInfoJSON {
                        kind: variable.name.clone(),
                        named: true,
                        root: false,
                        extra: extra_names.contains(&variable.name),
                        fields: None,
                        children: None,
                        subtypes: None,
                    });
            let mut subtypes = info
                .children
                .types
                .iter()
                .map(child_type_to_node_type)
                .collect::<Vec<_>>();
            subtypes.sort_unstable();
            subtypes.dedup();
            let supertype = NodeTypeJSON {
                kind: node_type_json.kind.clone(),
                named: true,
            };
            subtype_map.push((supertype, subtypes.clone()));
            node_type_json.subtypes = Some(subtypes);
        } else if !syntax_grammar.variables_to_inline.contains(&symbol) {
            // If a rule is aliased under multiple names, then its information
            // contributes to multiple entries in the final JSON.
            for alias in aliases_by_symbol.get(&symbol).unwrap_or(&HashSet::new()) {
                let kind;
                let is_named;
                if let Some(alias) = alias {
                    kind = &alias.value;
                    is_named = alias.is_named;
                } else if variable.kind.is_visible() {
                    kind = &variable.name;
                    is_named = variable.kind == VariableType::Named;
                } else {
                    continue;
                }

                // There may already be an entry with this name, because multiple
                // rules may be aliased with the same name.
                let mut node_type_existed = true;
                let node_type_json = node_types_json.entry(kind.clone()).or_insert_with(|| {
                    node_type_existed = false;
                    NodeInfoJSON {
                        kind: kind.clone(),
                        named: is_named,
                        root: i == 0,
                        extra: extra_names.contains(&kind),
                        fields: Some(BTreeMap::new()),
                        children: None,
                        subtypes: None,
                    }
                });

                let fields_json = node_type_json.fields.as_mut().unwrap();
                for (new_field, field_info) in &info.fields {
                    let field_json = fields_json.entry(new_field.clone()).or_insert_with(|| {
                        // If another rule is aliased with the same name, and does *not* have this
                        // field, then this field cannot be required.
                        let mut field_json = FieldInfoJSON::default();
                        if node_type_existed {
                            field_json.required = false;
                        }
                        field_json
                    });
                    populate_field_info_json(field_json, field_info);
                }

                // If another rule is aliased with the same name, any fields that aren't present in
                // this cannot be required.
                for (existing_field, field_json) in fields_json.iter_mut() {
                    if !info.fields.contains_key(existing_field) {
                        field_json.required = false;
                    }
                }

                populate_field_info_json(
                    node_type_json
                        .children
                        .get_or_insert(FieldInfoJSON::default()),
                    &info.children_without_fields,
                );
            }
        }
    }

    // Sort the subtype map topologically so that subtypes are listed before their supertypes.
    let mut sorted_kinds = Vec::with_capacity(subtype_map.len());
    let mut top_sort = topological_sort::TopologicalSort::<String>::new();
    for (supertype, subtypes) in &subtype_map {
        for subtype in subtypes {
            top_sort.add_dependency(subtype.kind.clone(), supertype.kind.clone());
        }
    }
    loop {
        let mut next_kinds = top_sort.pop_all();
        match (next_kinds.is_empty(), top_sort.is_empty()) {
            (true, true) => break,
            (true, false) => {
                let mut items = top_sort.collect::<Vec<String>>();
                items.sort();
                return Err(SuperTypeCycleError { items });
            }
            (false, _) => {
                next_kinds.sort();
                sorted_kinds.extend(next_kinds);
            }
        }
    }
    subtype_map.sort_by(|a, b| {
        let a_idx = sorted_kinds.iter().position(|n| n.eq(&a.0.kind)).unwrap();
        let b_idx = sorted_kinds.iter().position(|n| n.eq(&b.0.kind)).unwrap();
        a_idx.cmp(&b_idx)
    });

    for node_type_json in node_types_json.values_mut() {
        if node_type_json
            .children
            .as_ref()
            .is_some_and(|c| c.types.is_empty())
        {
            node_type_json.children = None;
        }

        if let Some(children) = &mut node_type_json.children {
            process_supertypes(children, &subtype_map);
        }
        if let Some(fields) = &mut node_type_json.fields {
            for field_info in fields.values_mut() {
                process_supertypes(field_info, &subtype_map);
            }
        }
    }

    let mut anonymous_node_types = Vec::new();

    let regular_tokens = lexical_grammar
        .variables
        .iter()
        .enumerate()
        .flat_map(|(i, variable)| {
            aliases_by_symbol
                .get(&Symbol::terminal(i))
                .unwrap_or(&empty)
                .iter()
                .map(move |alias| {
                    alias
                        .as_ref()
                        .map_or((&variable.name, variable.kind), |alias| {
                            (&alias.value, alias.kind())
                        })
                })
        });
    let external_tokens =
        syntax_grammar
            .external_tokens
            .iter()
            .enumerate()
            .flat_map(|(i, token)| {
                aliases_by_symbol
                    .get(&Symbol::external(i))
                    .unwrap_or(&empty)
                    .iter()
                    .map(move |alias| {
                        alias.as_ref().map_or((&token.name, token.kind), |alias| {
                            (&alias.value, alias.kind())
                        })
                    })
            });

    for (name, kind) in regular_tokens.chain(external_tokens) {
        match kind {
            VariableType::Named => {
                let node_type_json =
                    node_types_json
                        .entry(name.clone())
                        .or_insert_with(|| NodeInfoJSON {
                            kind: name.clone(),
                            named: true,
                            root: false,
                            extra: extra_names.contains(&name),
                            fields: None,
                            children: None,
                            subtypes: None,
                        });
                if let Some(children) = &mut node_type_json.children {
                    children.required = false;
                }
                if let Some(fields) = &mut node_type_json.fields {
                    for field in fields.values_mut() {
                        field.required = false;
                    }
                }
            }
            VariableType::Anonymous => anonymous_node_types.push(NodeInfoJSON {
                kind: name.clone(),
                named: false,
                root: false,
                extra: extra_names.contains(&name),
                fields: None,
                children: None,
                subtypes: None,
            }),
            _ => {}
        }
    }

    let mut result = node_types_json.into_iter().map(|e| e.1).collect::<Vec<_>>();
    result.extend(anonymous_node_types);
    result.sort_unstable_by(|a, b| {
        b.subtypes
            .is_some()
            .cmp(&a.subtypes.is_some())
            .then_with(|| {
                let a_is_leaf = a.children.is_none() && a.fields.is_none();
                let b_is_leaf = b.children.is_none() && b.fields.is_none();
                a_is_leaf.cmp(&b_is_leaf)
            })
            .then_with(|| a.kind.cmp(&b.kind))
    });
    result.dedup();
    Ok(result)
}

fn process_supertypes(info: &mut FieldInfoJSON, subtype_map: &[(NodeTypeJSON, Vec<NodeTypeJSON>)]) {
    for (supertype, subtypes) in subtype_map {
        if info.types.contains(supertype) {
            info.types.retain(|t| !subtypes.contains(t));
        }
    }
}

fn variable_type_for_child_type(
    child_type: &ChildType,
    syntax_grammar: &SyntaxGrammar,
    lexical_grammar: &LexicalGrammar,
) -> VariableType {
    match child_type {
        ChildType::Aliased(alias) => alias.kind(),
        ChildType::Normal(symbol) => {
            if syntax_grammar.supertype_symbols.contains(symbol) {
                VariableType::Named
            } else if syntax_grammar.variables_to_inline.contains(symbol) {
                VariableType::Hidden
            } else {
                match symbol.kind {
                    SymbolType::NonTerminal => syntax_grammar.variables[symbol.index].kind,
                    SymbolType::Terminal => lexical_grammar.variables[symbol.index].kind,
                    SymbolType::External => syntax_grammar.external_tokens[symbol.index].kind,
                    _ => VariableType::Hidden,
                }
            }
        }
    }
}

fn extend_sorted<'a, T>(vec: &mut Vec<T>, values: impl IntoIterator<Item = &'a T>) -> bool
where
    T: 'a + Clone + Eq + Ord,
{
    values.into_iter().any(|value| {
        if let Err(i) = vec.binary_search(value) {
            vec.insert(i, value.clone());
            true
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        grammars::{
            InputGrammar, LexicalVariable, Production, ProductionStep, SyntaxVariable, Variable,
        },
        prepare_grammar::prepare_grammar,
        rules::Rule,
    };

    #[test]
    fn test_node_types_simple() {
        let node_types = get_node_types(&InputGrammar {
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::field("f1".to_string(), Rule::named("v2")),
                        Rule::field("f2".to_string(), Rule::string(";")),
                    ]),
                },
                Variable {
                    name: "v2".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("x"),
                },
                // This rule is not reachable from the start symbol
                // so it won't be present in the node_types
                Variable {
                    name: "v3".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("y"),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(node_types.len(), 3);

        assert_eq!(
            node_types[0],
            NodeInfoJSON {
                kind: "v1".to_string(),
                named: true,
                root: true,
                extra: false,
                subtypes: None,
                children: None,
                fields: Some(
                    vec![
                        (
                            "f1".to_string(),
                            FieldInfoJSON {
                                multiple: false,
                                required: true,
                                types: vec![NodeTypeJSON {
                                    kind: "v2".to_string(),
                                    named: true,
                                }]
                            }
                        ),
                        (
                            "f2".to_string(),
                            FieldInfoJSON {
                                multiple: false,
                                required: true,
                                types: vec![NodeTypeJSON {
                                    kind: ";".to_string(),
                                    named: false,
                                }]
                            }
                        ),
                    ]
                    .into_iter()
                    .collect()
                )
            }
        );
        assert_eq!(
            node_types[1],
            NodeInfoJSON {
                kind: ";".to_string(),
                named: false,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None
            }
        );
        assert_eq!(
            node_types[2],
            NodeInfoJSON {
                kind: "v2".to_string(),
                named: true,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None
            }
        );
    }

    #[test]
    fn test_node_types_simple_extras() {
        let node_types = get_node_types(&InputGrammar {
            extra_symbols: vec![Rule::named("v3")],
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::field("f1".to_string(), Rule::named("v2")),
                        Rule::field("f2".to_string(), Rule::string(";")),
                    ]),
                },
                Variable {
                    name: "v2".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("x"),
                },
                // This rule is not reachable from the start symbol, but
                // it is reachable from the 'extra_symbols' so it
                // should be present in the node_types.
                // But because it's only a literal, it will get replaced by
                // a lexical variable.
                Variable {
                    name: "v3".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("y"),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(node_types.len(), 4);

        assert_eq!(
            node_types[0],
            NodeInfoJSON {
                kind: "v1".to_string(),
                named: true,
                root: true,
                extra: false,
                subtypes: None,
                children: None,
                fields: Some(
                    vec![
                        (
                            "f1".to_string(),
                            FieldInfoJSON {
                                multiple: false,
                                required: true,
                                types: vec![NodeTypeJSON {
                                    kind: "v2".to_string(),
                                    named: true,
                                }]
                            }
                        ),
                        (
                            "f2".to_string(),
                            FieldInfoJSON {
                                multiple: false,
                                required: true,
                                types: vec![NodeTypeJSON {
                                    kind: ";".to_string(),
                                    named: false,
                                }]
                            }
                        ),
                    ]
                    .into_iter()
                    .collect()
                )
            }
        );
        assert_eq!(
            node_types[1],
            NodeInfoJSON {
                kind: ";".to_string(),
                named: false,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None
            }
        );
        assert_eq!(
            node_types[2],
            NodeInfoJSON {
                kind: "v2".to_string(),
                named: true,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None
            }
        );
        assert_eq!(
            node_types[3],
            NodeInfoJSON {
                kind: "v3".to_string(),
                named: true,
                root: false,
                extra: true,
                subtypes: None,
                children: None,
                fields: None
            }
        );
    }

    #[test]
    fn test_node_types_deeper_extras() {
        let node_types = get_node_types(&InputGrammar {
            extra_symbols: vec![Rule::named("v3")],
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::field("f1".to_string(), Rule::named("v2")),
                        Rule::field("f2".to_string(), Rule::string(";")),
                    ]),
                },
                Variable {
                    name: "v2".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("x"),
                },
                // This rule is not reachable from the start symbol, but
                // it is reachable from the 'extra_symbols' so it
                // should be present in the node_types.
                // Because it is not just a literal, it won't get replaced
                // by a lexical variable.
                Variable {
                    name: "v3".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![Rule::string("y"), Rule::repeat(Rule::string("z"))]),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(node_types.len(), 6);

        assert_eq!(
            node_types[0],
            NodeInfoJSON {
                kind: "v1".to_string(),
                named: true,
                root: true,
                extra: false,
                subtypes: None,
                children: None,
                fields: Some(
                    vec![
                        (
                            "f1".to_string(),
                            FieldInfoJSON {
                                multiple: false,
                                required: true,
                                types: vec![NodeTypeJSON {
                                    kind: "v2".to_string(),
                                    named: true,
                                }]
                            }
                        ),
                        (
                            "f2".to_string(),
                            FieldInfoJSON {
                                multiple: false,
                                required: true,
                                types: vec![NodeTypeJSON {
                                    kind: ";".to_string(),
                                    named: false,
                                }]
                            }
                        ),
                    ]
                    .into_iter()
                    .collect()
                )
            }
        );
        assert_eq!(
            node_types[1],
            NodeInfoJSON {
                kind: "v3".to_string(),
                named: true,
                root: false,
                extra: true,
                subtypes: None,
                children: None,
                fields: Some(BTreeMap::default())
            }
        );
        assert_eq!(
            node_types[2],
            NodeInfoJSON {
                kind: ";".to_string(),
                named: false,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None
            }
        );
        assert_eq!(
            node_types[3],
            NodeInfoJSON {
                kind: "v2".to_string(),
                named: true,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None
            }
        );
    }

    #[test]
    fn test_node_types_with_supertypes() {
        let node_types = get_node_types(&InputGrammar {
            supertype_symbols: vec!["_v2".to_string()],
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::field("f1".to_string(), Rule::named("_v2")),
                },
                Variable {
                    name: "_v2".to_string(),
                    kind: VariableType::Hidden,
                    rule: Rule::choice(vec![
                        Rule::named("v3"),
                        Rule::named("v4"),
                        Rule::string("*"),
                    ]),
                },
                Variable {
                    name: "v3".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("x"),
                },
                Variable {
                    name: "v4".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("y"),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(
            node_types[0],
            NodeInfoJSON {
                kind: "_v2".to_string(),
                named: true,
                root: false,
                extra: false,
                fields: None,
                children: None,
                subtypes: Some(vec![
                    NodeTypeJSON {
                        kind: "*".to_string(),
                        named: false,
                    },
                    NodeTypeJSON {
                        kind: "v3".to_string(),
                        named: true,
                    },
                    NodeTypeJSON {
                        kind: "v4".to_string(),
                        named: true,
                    },
                ]),
            }
        );
        assert_eq!(
            node_types[1],
            NodeInfoJSON {
                kind: "v1".to_string(),
                named: true,
                root: true,
                extra: false,
                subtypes: None,
                children: None,
                fields: Some(
                    vec![(
                        "f1".to_string(),
                        FieldInfoJSON {
                            multiple: false,
                            required: true,
                            types: vec![NodeTypeJSON {
                                kind: "_v2".to_string(),
                                named: true,
                            }]
                        }
                    ),]
                    .into_iter()
                    .collect()
                )
            }
        );
    }

    #[test]
    fn test_node_types_for_children_without_fields() {
        let node_types = get_node_types(&InputGrammar {
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::named("v2"),
                        Rule::field("f1".to_string(), Rule::named("v3")),
                        Rule::named("v4"),
                    ]),
                },
                Variable {
                    name: "v2".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::string("{"),
                        Rule::choice(vec![Rule::named("v3"), Rule::Blank]),
                        Rule::string("}"),
                    ]),
                },
                Variable {
                    name: "v3".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("x"),
                },
                Variable {
                    name: "v4".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("y"),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(
            node_types[0],
            NodeInfoJSON {
                kind: "v1".to_string(),
                named: true,
                root: true,
                extra: false,
                subtypes: None,
                children: Some(FieldInfoJSON {
                    multiple: true,
                    required: true,
                    types: vec![
                        NodeTypeJSON {
                            kind: "v2".to_string(),
                            named: true,
                        },
                        NodeTypeJSON {
                            kind: "v4".to_string(),
                            named: true,
                        },
                    ]
                }),
                fields: Some(
                    vec![(
                        "f1".to_string(),
                        FieldInfoJSON {
                            multiple: false,
                            required: true,
                            types: vec![NodeTypeJSON {
                                kind: "v3".to_string(),
                                named: true,
                            }]
                        }
                    ),]
                    .into_iter()
                    .collect()
                )
            }
        );
        assert_eq!(
            node_types[1],
            NodeInfoJSON {
                kind: "v2".to_string(),
                named: true,
                root: false,
                extra: false,
                subtypes: None,
                children: Some(FieldInfoJSON {
                    multiple: false,
                    required: false,
                    types: vec![NodeTypeJSON {
                        kind: "v3".to_string(),
                        named: true,
                    },]
                }),
                fields: Some(BTreeMap::new()),
            }
        );
    }

    #[test]
    fn test_node_types_with_inlined_rules() {
        let node_types = get_node_types(&InputGrammar {
            variables_to_inline: vec!["v2".to_string()],
            variables: vec![
                Variable {
                    name: "v1".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![Rule::named("v2"), Rule::named("v3")]),
                },
                // v2 should not appear in the node types, since it is inlined
                Variable {
                    name: "v2".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::alias(Rule::string("a"), "x".to_string(), true),
                },
                Variable {
                    name: "v3".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("b"),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(
            node_types[0],
            NodeInfoJSON {
                kind: "v1".to_string(),
                named: true,
                root: true,
                extra: false,
                subtypes: None,
                children: Some(FieldInfoJSON {
                    multiple: true,
                    required: true,
                    types: vec![
                        NodeTypeJSON {
                            kind: "v3".to_string(),
                            named: true,
                        },
                        NodeTypeJSON {
                            kind: "x".to_string(),
                            named: true,
                        },
                    ]
                }),
                fields: Some(BTreeMap::new()),
            }
        );
    }

    #[test]
    fn test_node_types_for_aliased_nodes() {
        let node_types = get_node_types(&InputGrammar {
            variables: vec![
                Variable {
                    name: "thing".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::choice(vec![Rule::named("type"), Rule::named("expression")]),
                },
                Variable {
                    name: "type".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::choice(vec![
                        Rule::alias(
                            Rule::named("identifier"),
                            "type_identifier".to_string(),
                            true,
                        ),
                        Rule::string("void"),
                    ]),
                },
                Variable {
                    name: "expression".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::choice(vec![
                        Rule::named("identifier"),
                        Rule::alias(
                            Rule::named("foo_identifier"),
                            "identifier".to_string(),
                            true,
                        ),
                    ]),
                },
                Variable {
                    name: "identifier".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::pattern("\\w+", ""),
                },
                Variable {
                    name: "foo_identifier".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::pattern("[\\w-]+", ""),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(node_types.iter().find(|t| t.kind == "foo_identifier"), None);
        assert_eq!(
            node_types.iter().find(|t| t.kind == "identifier"),
            Some(&NodeInfoJSON {
                kind: "identifier".to_string(),
                named: true,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None,
            })
        );
        assert_eq!(
            node_types.iter().find(|t| t.kind == "type_identifier"),
            Some(&NodeInfoJSON {
                kind: "type_identifier".to_string(),
                named: true,
                root: false,
                extra: false,
                subtypes: None,
                children: None,
                fields: None,
            })
        );
    }

    #[test]
    fn test_node_types_with_multiple_valued_fields() {
        let node_types = get_node_types(&InputGrammar {
            variables: vec![
                Variable {
                    name: "a".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::choice(vec![
                            Rule::Blank,
                            Rule::repeat(Rule::field("f1".to_string(), Rule::named("b"))),
                        ]),
                        Rule::repeat(Rule::named("c")),
                    ]),
                },
                Variable {
                    name: "b".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("b"),
                },
                Variable {
                    name: "c".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::string("c"),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(
            node_types[0],
            NodeInfoJSON {
                kind: "a".to_string(),
                named: true,
                root: true,
                extra: false,
                subtypes: None,
                children: Some(FieldInfoJSON {
                    multiple: true,
                    required: true,
                    types: vec![NodeTypeJSON {
                        kind: "c".to_string(),
                        named: true,
                    },]
                }),
                fields: Some(
                    vec![(
                        "f1".to_string(),
                        FieldInfoJSON {
                            multiple: true,
                            required: false,
                            types: vec![NodeTypeJSON {
                                kind: "b".to_string(),
                                named: true,
                            }]
                        }
                    )]
                    .into_iter()
                    .collect()
                ),
            }
        );
    }

    #[test]
    fn test_node_types_with_fields_on_hidden_tokens() {
        let node_types = get_node_types(&InputGrammar {
            variables: vec![Variable {
                name: "script".to_string(),
                kind: VariableType::Named,
                rule: Rule::seq(vec![
                    Rule::field("a".to_string(), Rule::pattern("hi", "")),
                    Rule::field("b".to_string(), Rule::pattern("bye", "")),
                ]),
            }],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(
            node_types,
            [NodeInfoJSON {
                kind: "script".to_string(),
                named: true,
                root: true,
                extra: false,
                fields: Some(BTreeMap::new()),
                children: None,
                subtypes: None
            }]
        );
    }

    #[test]
    fn test_node_types_with_multiple_rules_same_alias_name() {
        let node_types = get_node_types(&InputGrammar {
            variables: vec![
                Variable {
                    name: "script".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::choice(vec![
                        Rule::named("a"),
                        // Rule `b` is aliased as rule `a`
                        Rule::alias(Rule::named("b"), "a".to_string(), true),
                    ]),
                },
                Variable {
                    name: "a".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::field("f1".to_string(), Rule::string("1")),
                        Rule::field("f2".to_string(), Rule::string("2")),
                    ]),
                },
                Variable {
                    name: "b".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![
                        Rule::field("f2".to_string(), Rule::string("22")),
                        Rule::field("f2".to_string(), Rule::string("222")),
                        Rule::field("f3".to_string(), Rule::string("3")),
                    ]),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(
            &node_types
                .iter()
                .map(|t| t.kind.as_str())
                .collect::<Vec<_>>(),
            &["a", "script", "1", "2", "22", "222", "3"]
        );

        assert_eq!(
            &node_types[0..2],
            &[
                // A combination of the types for `a` and `b`.
                NodeInfoJSON {
                    kind: "a".to_string(),
                    named: true,
                    root: false,
                    extra: false,
                    subtypes: None,
                    children: None,
                    fields: Some(
                        vec![
                            (
                                "f1".to_string(),
                                FieldInfoJSON {
                                    multiple: false,
                                    required: false,
                                    types: vec![NodeTypeJSON {
                                        kind: "1".to_string(),
                                        named: false,
                                    }]
                                }
                            ),
                            (
                                "f2".to_string(),
                                FieldInfoJSON {
                                    multiple: true,
                                    required: true,
                                    types: vec![
                                        NodeTypeJSON {
                                            kind: "2".to_string(),
                                            named: false,
                                        },
                                        NodeTypeJSON {
                                            kind: "22".to_string(),
                                            named: false,
                                        },
                                        NodeTypeJSON {
                                            kind: "222".to_string(),
                                            named: false,
                                        }
                                    ]
                                },
                            ),
                            (
                                "f3".to_string(),
                                FieldInfoJSON {
                                    multiple: false,
                                    required: false,
                                    types: vec![NodeTypeJSON {
                                        kind: "3".to_string(),
                                        named: false,
                                    }]
                                }
                            ),
                        ]
                        .into_iter()
                        .collect()
                    ),
                },
                NodeInfoJSON {
                    kind: "script".to_string(),
                    named: true,
                    root: true,
                    extra: false,
                    subtypes: None,
                    // Only one node
                    children: Some(FieldInfoJSON {
                        multiple: false,
                        required: true,
                        types: vec![NodeTypeJSON {
                            kind: "a".to_string(),
                            named: true,
                        }]
                    }),
                    fields: Some(BTreeMap::new()),
                }
            ]
        );
    }

    #[test]
    fn test_node_types_with_tokens_aliased_to_match_rules() {
        let node_types = get_node_types(&InputGrammar {
            variables: vec![
                Variable {
                    name: "a".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![Rule::named("b"), Rule::named("c")]),
                },
                // Ordinarily, `b` nodes have two named `c` children.
                Variable {
                    name: "b".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::seq(vec![Rule::named("c"), Rule::string("B"), Rule::named("c")]),
                },
                Variable {
                    name: "c".to_string(),
                    kind: VariableType::Named,
                    rule: Rule::choice(vec![
                        Rule::string("C"),
                        // This token is aliased as a `b`, which will produce a `b` node
                        // with no children.
                        Rule::alias(Rule::string("D"), "b".to_string(), true),
                    ]),
                },
            ],
            ..Default::default()
        })
        .unwrap();

        assert_eq!(
            node_types.iter().map(|n| &n.kind).collect::<Vec<_>>(),
            &["a", "b", "c", "B", "C"]
        );
        assert_eq!(
            node_types[1],
            NodeInfoJSON {
                kind: "b".to_string(),
                named: true,
                root: false,
                extra: false,
                subtypes: None,
                children: Some(FieldInfoJSON {
                    multiple: true,
                    required: false,
                    types: vec![NodeTypeJSON {
                        kind: "c".to_string(),
                        named: true,
                    }]
                }),
                fields: Some(BTreeMap::new()),
            }
        );
    }

    #[test]
    fn test_get_variable_info() {
        let variable_info = get_variable_info(
            &build_syntax_grammar(
                vec![
                    // Required field `field1` has only one node type.
                    SyntaxVariable {
                        name: "rule0".to_string(),
                        kind: VariableType::Named,
                        productions: vec![Production {
                            dynamic_precedence: 0,
                            steps: vec![
                                ProductionStep::new(Symbol::terminal(0)),
                                ProductionStep::new(Symbol::non_terminal(1))
                                    .with_field_name("field1"),
                            ],
                        }],
                    },
                    // Hidden node
                    SyntaxVariable {
                        name: "_rule1".to_string(),
                        kind: VariableType::Hidden,
                        productions: vec![Production {
                            dynamic_precedence: 0,
                            steps: vec![ProductionStep::new(Symbol::terminal(1))],
                        }],
                    },
                    // Optional field `field2` can have two possible node types.
                    SyntaxVariable {
                        name: "rule2".to_string(),
                        kind: VariableType::Named,
                        productions: vec![
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![ProductionStep::new(Symbol::terminal(0))],
                            },
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![
                                    ProductionStep::new(Symbol::terminal(0)),
                                    ProductionStep::new(Symbol::terminal(2))
                                        .with_field_name("field2"),
                                ],
                            },
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![
                                    ProductionStep::new(Symbol::terminal(0)),
                                    ProductionStep::new(Symbol::terminal(3))
                                        .with_field_name("field2"),
                                ],
                            },
                        ],
                    },
                ],
                vec![],
            ),
            &build_lexical_grammar(),
            &AliasMap::new(),
        )
        .unwrap();

        assert_eq!(
            variable_info[0].fields,
            vec![(
                "field1".to_string(),
                FieldInfo {
                    quantity: ChildQuantity {
                        exists: true,
                        required: true,
                        multiple: false,
                    },
                    types: vec![ChildType::Normal(Symbol::terminal(1))],
                }
            )]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );

        assert_eq!(
            variable_info[2].fields,
            vec![(
                "field2".to_string(),
                FieldInfo {
                    quantity: ChildQuantity {
                        exists: true,
                        required: false,
                        multiple: false,
                    },
                    types: vec![
                        ChildType::Normal(Symbol::terminal(2)),
                        ChildType::Normal(Symbol::terminal(3)),
                    ],
                }
            )]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );
    }

    #[test]
    fn test_get_variable_info_with_repetitions_inside_fields() {
        let variable_info = get_variable_info(
            &build_syntax_grammar(
                vec![
                    // Field associated with a repetition.
                    SyntaxVariable {
                        name: "rule0".to_string(),
                        kind: VariableType::Named,
                        productions: vec![
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![ProductionStep::new(Symbol::non_terminal(1))
                                    .with_field_name("field1")],
                            },
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![],
                            },
                        ],
                    },
                    // Repetition node
                    SyntaxVariable {
                        name: "_rule0_repeat".to_string(),
                        kind: VariableType::Hidden,
                        productions: vec![
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![ProductionStep::new(Symbol::terminal(1))],
                            },
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![
                                    ProductionStep::new(Symbol::non_terminal(1)),
                                    ProductionStep::new(Symbol::non_terminal(1)),
                                ],
                            },
                        ],
                    },
                ],
                vec![],
            ),
            &build_lexical_grammar(),
            &AliasMap::new(),
        )
        .unwrap();

        assert_eq!(
            variable_info[0].fields,
            vec![(
                "field1".to_string(),
                FieldInfo {
                    quantity: ChildQuantity {
                        exists: true,
                        required: false,
                        multiple: true,
                    },
                    types: vec![ChildType::Normal(Symbol::terminal(1))],
                }
            )]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );
    }

    #[test]
    fn test_get_variable_info_with_inherited_fields() {
        let variable_info = get_variable_info(
            &build_syntax_grammar(
                vec![
                    SyntaxVariable {
                        name: "rule0".to_string(),
                        kind: VariableType::Named,
                        productions: vec![
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![
                                    ProductionStep::new(Symbol::terminal(0)),
                                    ProductionStep::new(Symbol::non_terminal(1)),
                                    ProductionStep::new(Symbol::terminal(1)),
                                ],
                            },
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![ProductionStep::new(Symbol::non_terminal(1))],
                            },
                        ],
                    },
                    // Hidden node with fields
                    SyntaxVariable {
                        name: "_rule1".to_string(),
                        kind: VariableType::Hidden,
                        productions: vec![Production {
                            dynamic_precedence: 0,
                            steps: vec![
                                ProductionStep::new(Symbol::terminal(2)).with_alias(".", false),
                                ProductionStep::new(Symbol::terminal(3)).with_field_name("field1"),
                            ],
                        }],
                    },
                ],
                vec![],
            ),
            &build_lexical_grammar(),
            &AliasMap::new(),
        )
        .unwrap();

        assert_eq!(
            variable_info[0].fields,
            vec![(
                "field1".to_string(),
                FieldInfo {
                    quantity: ChildQuantity {
                        exists: true,
                        required: true,
                        multiple: false,
                    },
                    types: vec![ChildType::Normal(Symbol::terminal(3))],
                }
            )]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );

        assert_eq!(
            variable_info[0].children_without_fields,
            FieldInfo {
                quantity: ChildQuantity {
                    exists: true,
                    required: false,
                    multiple: true,
                },
                types: vec![
                    ChildType::Normal(Symbol::terminal(0)),
                    ChildType::Normal(Symbol::terminal(1)),
                ],
            }
        );
    }

    #[test]
    fn test_get_variable_info_with_supertypes() {
        let variable_info = get_variable_info(
            &build_syntax_grammar(
                vec![
                    SyntaxVariable {
                        name: "rule0".to_string(),
                        kind: VariableType::Named,
                        productions: vec![Production {
                            dynamic_precedence: 0,
                            steps: vec![
                                ProductionStep::new(Symbol::terminal(0)),
                                ProductionStep::new(Symbol::non_terminal(1))
                                    .with_field_name("field1"),
                                ProductionStep::new(Symbol::terminal(1)),
                            ],
                        }],
                    },
                    SyntaxVariable {
                        name: "_rule1".to_string(),
                        kind: VariableType::Hidden,
                        productions: vec![
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![ProductionStep::new(Symbol::terminal(2))],
                            },
                            Production {
                                dynamic_precedence: 0,
                                steps: vec![ProductionStep::new(Symbol::terminal(3))],
                            },
                        ],
                    },
                ],
                // _rule1 is a supertype
                vec![Symbol::non_terminal(1)],
            ),
            &build_lexical_grammar(),
            &AliasMap::new(),
        )
        .unwrap();

        assert_eq!(
            variable_info[0].fields,
            vec![(
                "field1".to_string(),
                FieldInfo {
                    quantity: ChildQuantity {
                        exists: true,
                        required: true,
                        multiple: false,
                    },
                    types: vec![ChildType::Normal(Symbol::non_terminal(1))],
                }
            )]
            .into_iter()
            .collect::<HashMap<_, _>>()
        );
    }

    fn get_node_types(grammar: &InputGrammar) -> SuperTypeCycleResult<Vec<NodeInfoJSON>> {
        let (syntax_grammar, lexical_grammar, _, default_aliases) =
            prepare_grammar(grammar).unwrap();
        let variable_info =
            get_variable_info(&syntax_grammar, &lexical_grammar, &default_aliases).unwrap();
        generate_node_types_json(
            &syntax_grammar,
            &lexical_grammar,
            &default_aliases,
            &variable_info,
        )
    }

    fn build_syntax_grammar(
        variables: Vec<SyntaxVariable>,
        supertype_symbols: Vec<Symbol>,
    ) -> SyntaxGrammar {
        SyntaxGrammar {
            variables,
            supertype_symbols,
            ..SyntaxGrammar::default()
        }
    }

    fn build_lexical_grammar() -> LexicalGrammar {
        let mut lexical_grammar = LexicalGrammar::default();
        for i in 0..10 {
            lexical_grammar.variables.push(LexicalVariable {
                name: format!("token_{i}"),
                kind: VariableType::Named,
                implicit_precedence: 0,
                start_state: 0,
            });
        }
        lexical_grammar
    }
}
