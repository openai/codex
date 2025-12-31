//! Symbol resolution with AI-friendly name+kind matching

use lsp_types::DocumentSymbol;
use lsp_types::DocumentSymbolResponse;
use lsp_types::Position;
use lsp_types::SymbolKind as LspSymbolKind;
use serde::Deserialize;
use serde::Serialize;

/// Simplified symbol kind for AI consumption
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Enum,
    Variable,
    Constant,
    Property,
    Field,
    Module,
    Type,
    Other,
}

impl From<LspSymbolKind> for SymbolKind {
    fn from(kind: LspSymbolKind) -> Self {
        match kind {
            LspSymbolKind::FUNCTION => SymbolKind::Function,
            LspSymbolKind::METHOD => SymbolKind::Method,
            LspSymbolKind::CLASS => SymbolKind::Class,
            LspSymbolKind::STRUCT => SymbolKind::Struct,
            LspSymbolKind::INTERFACE => SymbolKind::Interface,
            LspSymbolKind::ENUM => SymbolKind::Enum,
            LspSymbolKind::VARIABLE => SymbolKind::Variable,
            LspSymbolKind::CONSTANT => SymbolKind::Constant,
            LspSymbolKind::PROPERTY => SymbolKind::Property,
            LspSymbolKind::FIELD => SymbolKind::Field,
            LspSymbolKind::MODULE | LspSymbolKind::NAMESPACE => SymbolKind::Module,
            LspSymbolKind::TYPE_PARAMETER => SymbolKind::Type,
            _ => SymbolKind::Other,
        }
    }
}

impl SymbolKind {
    /// Parse from string (case-insensitive, loose matching)
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "function" | "func" | "fn" => Some(SymbolKind::Function),
            "method" => Some(SymbolKind::Method),
            "class" => Some(SymbolKind::Class),
            "struct" => Some(SymbolKind::Struct),
            "interface" | "trait" => Some(SymbolKind::Interface),
            "enum" => Some(SymbolKind::Enum),
            "variable" | "var" | "let" => Some(SymbolKind::Variable),
            "constant" | "const" => Some(SymbolKind::Constant),
            "property" | "prop" => Some(SymbolKind::Property),
            "field" => Some(SymbolKind::Field),
            "module" | "mod" | "namespace" => Some(SymbolKind::Module),
            "type" => Some(SymbolKind::Type),
            _ => None,
        }
    }

    /// Get display name for the symbol kind
    pub fn display_name(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Interface => "interface",
            SymbolKind::Enum => "enum",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Property => "property",
            SymbolKind::Field => "field",
            SymbolKind::Module => "module",
            SymbolKind::Type => "type",
            SymbolKind::Other => "symbol",
        }
    }
}

/// Resolved symbol with position
#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub position: Position,
    pub range_start_line: i32,
    pub range_end_line: i32,
}

/// Symbol match result
#[derive(Debug, Clone)]
pub struct SymbolMatch {
    pub symbol: ResolvedSymbol,
    pub exact_name_match: bool,
}

/// Flatten document symbols (handles nested symbols)
pub fn flatten_symbols(response: &DocumentSymbolResponse) -> Vec<ResolvedSymbol> {
    let mut result = Vec::new();

    match response {
        DocumentSymbolResponse::Flat(symbols) => {
            for sym in symbols {
                result.push(ResolvedSymbol {
                    name: sym.name.clone(),
                    kind: sym.kind.into(),
                    position: sym.location.range.start,
                    range_start_line: sym.location.range.start.line as i32,
                    range_end_line: sym.location.range.end.line as i32,
                });
            }
        }
        DocumentSymbolResponse::Nested(symbols) => {
            flatten_nested(&mut result, symbols);
        }
    }

    result
}

fn flatten_nested(result: &mut Vec<ResolvedSymbol>, symbols: &[DocumentSymbol]) {
    for sym in symbols {
        result.push(ResolvedSymbol {
            name: sym.name.clone(),
            kind: sym.kind.into(),
            position: sym.selection_range.start,
            range_start_line: sym.range.start.line as i32,
            range_end_line: sym.range.end.line as i32,
        });

        // Recurse into children
        if let Some(children) = &sym.children {
            flatten_nested(result, children);
        }
    }
}

/// Find symbols matching name and optional kind
pub fn find_matching_symbols(
    symbols: &[ResolvedSymbol],
    name: &str,
    kind: Option<SymbolKind>,
) -> Vec<SymbolMatch> {
    // Lazily allocate lowercase name only when needed for substring matching
    let name_lower = std::cell::OnceCell::new();
    let get_name_lower = || name_lower.get_or_init(|| name.to_lowercase());

    let mut matches: Vec<SymbolMatch> = symbols
        .iter()
        .filter_map(|sym| {
            // Filter by kind first (cheap check, no allocation)
            if let Some(k) = kind {
                if sym.kind != k {
                    return None;
                }
            }

            // Fast path: case-insensitive exact match (no allocation)
            let exact_name_match = sym.name.eq_ignore_ascii_case(name);

            // Only perform substring match if not an exact match
            // This is the slow path that requires allocation
            if !exact_name_match {
                let sym_lower = sym.name.to_lowercase();
                if !sym_lower.contains(get_name_lower()) {
                    return None;
                }
            }

            Some(SymbolMatch {
                symbol: sym.clone(),
                exact_name_match,
            })
        })
        .collect();

    // Sort: exact matches first
    matches.sort_by(|a, b| b.exact_name_match.cmp(&a.exact_name_match));

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_from_str() {
        assert_eq!(
            SymbolKind::from_str_loose("function"),
            Some(SymbolKind::Function)
        );
        assert_eq!(SymbolKind::from_str_loose("fn"), Some(SymbolKind::Function));
        assert_eq!(
            SymbolKind::from_str_loose("STRUCT"),
            Some(SymbolKind::Struct)
        );
        assert_eq!(
            SymbolKind::from_str_loose("trait"),
            Some(SymbolKind::Interface)
        );
        assert_eq!(SymbolKind::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_symbol_kind_display_name() {
        assert_eq!(SymbolKind::Function.display_name(), "function");
        assert_eq!(SymbolKind::Struct.display_name(), "struct");
        assert_eq!(SymbolKind::Other.display_name(), "symbol");
    }

    #[test]
    fn test_find_matching_symbols() {
        let symbols = vec![
            ResolvedSymbol {
                name: "process_data".to_string(),
                kind: SymbolKind::Function,
                position: Position {
                    line: 10,
                    character: 0,
                },
                range_start_line: 10,
                range_end_line: 20,
            },
            ResolvedSymbol {
                name: "ProcessData".to_string(),
                kind: SymbolKind::Struct,
                position: Position {
                    line: 5,
                    character: 0,
                },
                range_start_line: 5,
                range_end_line: 8,
            },
            ResolvedSymbol {
                name: "other_func".to_string(),
                kind: SymbolKind::Function,
                position: Position {
                    line: 30,
                    character: 0,
                },
                range_start_line: 30,
                range_end_line: 35,
            },
        ];

        // Exact match with kind filter
        let matches = find_matching_symbols(&symbols, "process_data", Some(SymbolKind::Function));
        assert_eq!(matches.len(), 1);
        assert!(matches[0].exact_name_match);
        assert_eq!(matches[0].symbol.kind, SymbolKind::Function);

        // Case-insensitive matching without kind filter
        // "process_data" matches exactly, but "ProcessData" (lowercased "processdata")
        // does NOT contain "process_data" as substring due to underscore
        let matches = find_matching_symbols(&symbols, "PROCESS_DATA", None);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].exact_name_match);

        // Searching for "process" should match both (substring match)
        let matches = find_matching_symbols(&symbols, "process", None);
        assert_eq!(matches.len(), 2);

        // Kind filter narrows results
        let matches = find_matching_symbols(&symbols, "process", Some(SymbolKind::Struct));
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].symbol.name, "ProcessData");

        // No matches
        let matches = find_matching_symbols(&symbols, "nonexistent", None);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_matching_symbols_exact_first() {
        let symbols = vec![
            ResolvedSymbol {
                name: "Config".to_string(),
                kind: SymbolKind::Struct,
                position: Position {
                    line: 1,
                    character: 0,
                },
                range_start_line: 1,
                range_end_line: 5,
            },
            ResolvedSymbol {
                name: "ConfigBuilder".to_string(),
                kind: SymbolKind::Struct,
                position: Position {
                    line: 10,
                    character: 0,
                },
                range_start_line: 10,
                range_end_line: 20,
            },
        ];

        let matches = find_matching_symbols(&symbols, "config", None);
        assert_eq!(matches.len(), 2);
        // Exact match should be first
        assert!(matches[0].exact_name_match);
        assert_eq!(matches[0].symbol.name, "Config");
    }
}
