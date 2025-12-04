use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use tree_sitter::wasm_stdlib_symbols;
use tree_sitter_generate::parse_grammar::GrammarJSON;
use tree_sitter_loader::Loader;
use wasmparser::Parser;

pub fn load_language_wasm_file(language_dir: &Path) -> Result<(String, Vec<u8>)> {
    let grammar_name = get_grammar_name(language_dir)
        .with_context(|| "Failed to get wasm filename")
        .unwrap();
    let wasm_filename = format!("tree-sitter-{grammar_name}.wasm");
    let contents = fs::read(language_dir.join(&wasm_filename)).with_context(|| {
        format!("Failed to read {wasm_filename}. Run `tree-sitter build --wasm` first.",)
    })?;
    Ok((grammar_name, contents))
}

pub fn get_grammar_name(language_dir: &Path) -> Result<String> {
    let src_dir = language_dir.join("src");
    let grammar_json_path = src_dir.join("grammar.json");
    let grammar_json = fs::read_to_string(&grammar_json_path).with_context(|| {
        format!(
            "Failed to read grammar file {}",
            grammar_json_path.display()
        )
    })?;
    let grammar: GrammarJSON = serde_json::from_str(&grammar_json).with_context(|| {
        format!(
            "Failed to parse grammar file {}",
            grammar_json_path.display()
        )
    })?;
    Ok(grammar.name)
}

pub fn compile_language_to_wasm(
    loader: &Loader,
    root_dir: Option<&Path>,
    language_dir: &Path,
    output_dir: &Path,
    output_file: Option<PathBuf>,
    force_docker: bool,
) -> Result<()> {
    let grammar_name = get_grammar_name(language_dir)?;
    let output_filename =
        output_file.unwrap_or_else(|| output_dir.join(format!("tree-sitter-{grammar_name}.wasm")));
    let src_path = language_dir.join("src");
    let scanner_path = loader.get_scanner_path(&src_path);
    loader.compile_parser_to_wasm(
        &grammar_name,
        root_dir,
        &src_path,
        scanner_path
            .as_ref()
            .and_then(|p| Some(Path::new(p.file_name()?))),
        &output_filename,
        force_docker,
    )?;

    // Exit with an error if the external scanner uses symbols from the
    // C or C++ standard libraries that aren't available to wasm parsers.
    let stdlib_symbols = wasm_stdlib_symbols().collect::<Vec<_>>();
    let dylink_symbols = [
        "__indirect_function_table",
        "__memory_base",
        "__stack_pointer",
        "__table_base",
        "__table_base",
        "memory",
    ];
    let builtin_symbols = [
        "__assert_fail",
        "__cxa_atexit",
        "abort",
        "emscripten_notify_memory_growth",
        "tree_sitter_debug_message",
        "proc_exit",
    ];

    let mut missing_symbols = Vec::new();
    let wasm_bytes = fs::read(&output_filename)?;
    let parser = Parser::new(0);
    for payload in parser.parse_all(&wasm_bytes) {
        if let wasmparser::Payload::ImportSection(imports) = payload? {
            for import in imports {
                let import = import?.name;
                if !builtin_symbols.contains(&import)
                    && !stdlib_symbols.contains(&import)
                    && !dylink_symbols.contains(&import)
                {
                    missing_symbols.push(import);
                }
            }
        }
    }

    if !missing_symbols.is_empty() {
        Err(anyhow!(
            concat!(
                "This external scanner uses a symbol that isn't available to wasm parsers.\n",
                "\n",
                "Missing symbols:\n",
                "    {}\n",
                "\n",
                "Available symbols:\n",
                "    {}",
            ),
            missing_symbols.join("\n    "),
            stdlib_symbols.join("\n    ")
        ))?;
    }

    Ok(())
}
