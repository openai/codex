use std::{fs, sync::LazyLock};

use streaming_iterator::StreamingIterator;
use tree_sitter::{
    wasmtime::Engine, Parser, Query, QueryCursor, WasmError, WasmErrorKind, WasmStore,
};

use crate::tests::helpers::{allocations, fixtures::WASM_DIR};

static ENGINE: LazyLock<Engine> = LazyLock::new(Engine::default);

#[test]
fn test_wasm_stdlib_symbols() {
    let symbols = tree_sitter::wasm_stdlib_symbols().collect::<Vec<_>>();
    assert_eq!(
        symbols,
        {
            let mut symbols = symbols.clone();
            symbols.sort_unstable();
            symbols
        },
        "symbols aren't sorted"
    );

    assert!(symbols.contains(&"malloc"));
    assert!(symbols.contains(&"free"));
    assert!(symbols.contains(&"memset"));
    assert!(symbols.contains(&"memcpy"));
}

#[test]
fn test_load_wasm_ruby_language() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();
        let mut parser = Parser::new();
        let wasm = fs::read(WASM_DIR.join("tree-sitter-ruby.wasm")).unwrap();
        let language = store.load_language("ruby", &wasm).unwrap();
        parser.set_wasm_store(store).unwrap();
        parser.set_language(&language).unwrap();
        let tree = parser.parse("class A; end", None).unwrap();
        assert_eq!(
            tree.root_node().to_sexp(),
            "(program (class name: (constant)))"
        );
    });
}

#[test]
fn test_load_wasm_html_language() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();
        let mut parser = Parser::new();
        let wasm = fs::read(WASM_DIR.join("tree-sitter-html.wasm")).unwrap();
        let language = store.load_language("html", &wasm).unwrap();
        parser.set_wasm_store(store).unwrap();
        parser.set_language(&language).unwrap();
        let tree = parser
            .parse("<div><span></span><p></p></div>", None)
            .unwrap();
        assert_eq!(
            tree.root_node().to_sexp(),
            "(document (element (start_tag (tag_name)) (element (start_tag (tag_name)) (end_tag (tag_name))) (element (start_tag (tag_name)) (end_tag (tag_name))) (end_tag (tag_name))))"
        );
    });
}

#[test]
fn test_load_wasm_rust_language() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();
        let mut parser = Parser::new();
        let wasm = fs::read(WASM_DIR.join("tree-sitter-rust.wasm")).unwrap();
        let language = store.load_language("rust", &wasm).unwrap();
        parser.set_wasm_store(store).unwrap();
        parser.set_language(&language).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();
        assert_eq!(tree.root_node().to_sexp(), "(source_file (function_item name: (identifier) parameters: (parameters) body: (block)))");
    });
}

#[test]
fn test_load_wasm_javascript_language() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();
        let mut parser = Parser::new();
        let wasm = fs::read(WASM_DIR.join("tree-sitter-javascript.wasm")).unwrap();
        let language = store.load_language("javascript", &wasm).unwrap();
        parser.set_wasm_store(store).unwrap();
        parser.set_language(&language).unwrap();
        let tree = parser.parse("const a = b\nconst c = d", None).unwrap();
        assert_eq!(tree.root_node().to_sexp(), "(program (lexical_declaration (variable_declarator name: (identifier) value: (identifier))) (lexical_declaration (variable_declarator name: (identifier) value: (identifier))))");
    });
}

#[test]
fn test_load_multiple_wasm_languages() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();
        let mut parser = Parser::new();

        let wasm_cpp = fs::read(WASM_DIR.join("tree-sitter-cpp.wasm")).unwrap();
        let wasm_rs = fs::read(WASM_DIR.join("tree-sitter-rust.wasm")).unwrap();
        let wasm_rb = fs::read(WASM_DIR.join("tree-sitter-ruby.wasm")).unwrap();
        let wasm_typescript = fs::read(WASM_DIR.join("tree-sitter-typescript.wasm")).unwrap();

        let language_rust = store.load_language("rust", &wasm_rs).unwrap();
        let language_cpp = store.load_language("cpp", &wasm_cpp).unwrap();
        let language_ruby = store.load_language("ruby", &wasm_rb).unwrap();
        let language_typescript = store.load_language("typescript", &wasm_typescript).unwrap();
        parser.set_wasm_store(store).unwrap();

        let mut parser2 = Parser::new();
        parser2
            .set_wasm_store(WasmStore::new(&ENGINE).unwrap())
            .unwrap();
        let mut query_cursor = QueryCursor::new();

        // First, parse with the store that originally loaded the languages.
        // Then parse with a new parser and wasm store, so that the languages
        // are added one-by-one, in between parses.
        for mut parser in [parser, parser2] {
            for _ in 0..2 {
                let query_rust = Query::new(&language_rust, "(const_item) @foo").unwrap();
                let query_typescript =
                    Query::new(&language_typescript, "(class_declaration) @foo").unwrap();

                parser.set_language(&language_cpp).unwrap();
                let tree = parser.parse("A<B> c = d();", None).unwrap();
                assert_eq!(
                    tree.root_node().to_sexp(),
                    "(translation_unit (declaration type: (template_type name: (type_identifier) arguments: (template_argument_list (type_descriptor type: (type_identifier)))) declarator: (init_declarator declarator: (identifier) value: (call_expression function: (identifier) arguments: (argument_list)))))"
                );

                parser.set_language(&language_rust).unwrap();
                let source = "const A: B = c();";
                let tree = parser.parse(source, None).unwrap();
                assert_eq!(
                    tree.root_node().to_sexp(),
                    "(source_file (const_item name: (identifier) type: (type_identifier) value: (call_expression function: (identifier) arguments: (arguments))))"
                );
                assert_eq!(
                    query_cursor
                        .matches(&query_rust, tree.root_node(), source.as_bytes())
                        .count(),
                    1
                );

                parser.set_language(&language_ruby).unwrap();
                let tree = parser.parse("class A; end", None).unwrap();
                assert_eq!(
                    tree.root_node().to_sexp(),
                    "(program (class name: (constant)))"
                );

                parser.set_language(&language_typescript).unwrap();
                let tree = parser.parse("class A {}", None).unwrap();
                assert_eq!(
                    tree.root_node().to_sexp(),
                    "(program (class_declaration name: (type_identifier) body: (class_body)))"
                );
                assert_eq!(
                    query_cursor
                        .matches(&query_typescript, tree.root_node(), source.as_bytes())
                        .count(),
                    1
                );
            }
        }
    });
}

#[test]
fn test_load_and_reload_wasm_language() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();

        let wasm_rust = fs::read(WASM_DIR.join("tree-sitter-rust.wasm")).unwrap();
        let wasm_typescript = fs::read(WASM_DIR.join("tree-sitter-typescript.wasm")).unwrap();

        let language_rust = store.load_language("rust", &wasm_rust).unwrap();
        let language_typescript = store.load_language("typescript", &wasm_typescript).unwrap();
        assert_eq!(store.language_count(), 2);

        // When a language is dropped, stores can release their instances of that language.
        drop(language_rust);
        assert_eq!(store.language_count(), 1);

        let language_rust = store.load_language("rust", &wasm_rust).unwrap();
        assert_eq!(store.language_count(), 2);

        drop(language_rust);
        drop(language_typescript);
        assert_eq!(store.language_count(), 0);
    });
}

#[test]
fn test_reset_wasm_store() {
    allocations::record(|| {
        let mut language_store = WasmStore::new(&ENGINE).unwrap();
        let wasm = fs::read(WASM_DIR.join("tree-sitter-rust.wasm")).unwrap();
        let language = language_store.load_language("rust", &wasm).unwrap();

        let mut parser = Parser::new();
        let parser_store = WasmStore::new(&ENGINE).unwrap();
        parser.set_wasm_store(parser_store).unwrap();
        parser.set_language(&language).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();
        assert_eq!(tree.root_node().to_sexp(), "(source_file (function_item name: (identifier) parameters: (parameters) body: (block)))");

        let parser_store = WasmStore::new(&ENGINE).unwrap();
        parser.set_wasm_store(parser_store).unwrap();
        let tree = parser.parse("fn main() {}", None).unwrap();
        assert_eq!(tree.root_node().to_sexp(), "(source_file (function_item name: (identifier) parameters: (parameters) body: (block)))");
    });
}

#[test]
fn test_load_wasm_errors() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();
        let wasm = fs::read(WASM_DIR.join("tree-sitter-rust.wasm")).unwrap();

        let bad_wasm = &wasm[1..];
        assert_eq!(
            store.load_language("rust", bad_wasm).unwrap_err(),
            WasmError {
                kind: WasmErrorKind::Parse,
                message: "failed to parse dylink section of wasm module".into(),
            }
        );

        assert_eq!(
            store.load_language("not_rust", &wasm).unwrap_err(),
            WasmError {
                kind: WasmErrorKind::Instantiate,
                message: "module did not contain language function: tree_sitter_not_rust".into(),
            }
        );

        let mut bad_wasm = wasm.clone();
        bad_wasm[300..500].iter_mut().for_each(|b| *b = 0);
        assert_eq!(
            store.load_language("rust", &bad_wasm).unwrap_err().kind,
            WasmErrorKind::Compile,
        );
    });
}

#[test]
fn test_wasm_oom() {
    allocations::record(|| {
        let mut store = WasmStore::new(&ENGINE).unwrap();
        let mut parser = Parser::new();
        let wasm = fs::read(WASM_DIR.join("tree-sitter-html.wasm")).unwrap();
        let language = store.load_language("html", &wasm).unwrap();
        parser.set_wasm_store(store).unwrap();
        parser.set_language(&language).unwrap();

        let tag_name = "a-b".repeat(2 * 1024 * 1024);
        let code = format!("<{tag_name}>hello world</{tag_name}>");
        assert!(parser.parse(&code, None).is_none());

        let tag_name = "a-b".repeat(20);
        let code = format!("<{tag_name}>hello world</{tag_name}>");
        parser.set_language(&language).unwrap();
        let tree = parser.parse(&code, None).unwrap();
        assert_eq!(
            tree.root_node().to_sexp(),
            "(document (element (start_tag (tag_name)) (text) (end_tag (tag_name))))"
        );
    });
}
