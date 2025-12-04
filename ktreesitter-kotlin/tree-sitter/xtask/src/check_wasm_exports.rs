use std::{
    collections::HashSet,
    io::BufRead,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

use anyhow::{anyhow, Result};
use notify::{
    event::{AccessKind, AccessMode},
    EventKind, RecursiveMode,
};
use notify_debouncer_full::new_debouncer;

use crate::{bail_on_err, watch_wasm, CheckWasmExports};

const EXCLUDES: [&str; 27] = [
    // Unneeded because the JS side has its own way of implementing it
    "ts_node_child_by_field_name",
    "ts_node_edit",
    // Precomputed and stored in the JS side
    "ts_node_type",
    "ts_node_grammar_type",
    "ts_node_eq",
    "ts_tree_cursor_current_field_name",
    "ts_lookahead_iterator_current_symbol_name",
    // Not used in wasm
    "ts_init",
    "ts_set_allocator",
    "ts_parser_set_cancellation_flag",
    "ts_parser_cancellation_flag",
    "ts_parser_print_dot_graphs",
    "ts_tree_print_dot_graph",
    "ts_parser_set_wasm_store",
    "ts_parser_take_wasm_store",
    "ts_parser_language",
    "ts_node_language",
    "ts_tree_language",
    "ts_lookahead_iterator_language",
    "ts_parser_logger",
    "ts_parser_parse_string",
    "ts_parser_parse_string_encoding",
    // Query cursor is not managed by user in web bindings
    "ts_query_cursor_delete",
    "ts_query_cursor_timeout_micros",
    "ts_query_cursor_match_limit",
    "ts_query_cursor_remove_match",
    "ts_query_cursor_timeout_micros",
];

pub fn run(args: &CheckWasmExports) -> Result<()> {
    if args.watch {
        watch_wasm!(check_wasm_exports);
    } else {
        check_wasm_exports()?;
    }

    Ok(())
}

fn check_wasm_exports() -> Result<()> {
    let mut wasm_exports = std::fs::read_to_string("lib/binding_web/lib/exports.txt")?
        .lines()
        .map(|s| s.replace("_wasm", "").replace("byte", "index"))
        // remove leading and trailing quotes, trailing comma
        .map(|s| s[1..s.len() - 2].to_string())
        .collect::<HashSet<_>>();

    // Run wasm-objdump to see symbols used internally in binding.c but not exposed in any way.
    let wasm_objdump = Command::new("wasm-objdump")
        .args([
            "--details",
            "lib/binding_web/debug/tree-sitter.wasm",
            "--section",
            "Name",
        ])
        .output()
        .expect("Failed to run wasm-objdump");
    bail_on_err(&wasm_objdump, "Failed to run wasm-objdump")?;

    wasm_exports.extend(
        wasm_objdump
            .stdout
            .lines()
            .map_while(Result::ok)
            .skip_while(|line| !line.contains("- func"))
            .filter_map(|line| {
                if line.contains("func") {
                    if let Some(function) = line.split_whitespace().nth(2).map(String::from) {
                        let trimmed = function.trim_start_matches('<').trim_end_matches('>');
                        if trimmed.starts_with("ts") && !trimmed.contains("__") {
                            return Some(trimmed.to_string());
                        }
                    }
                }
                None
            }),
    );

    let nm_child = Command::new("nm")
        .arg("-W")
        .arg("-U")
        .arg("libtree-sitter.so")
        .stdout(Stdio::piped())
        .output()
        .expect("Failed to run nm");
    bail_on_err(&nm_child, "Failed to run nm")?;
    let export_reader = nm_child
        .stdout
        .lines()
        .map_while(Result::ok)
        .filter(|line| line.contains(" T "));

    let exports = export_reader
        .filter_map(|line| line.split_whitespace().nth(2).map(String::from))
        .filter(|symbol| !EXCLUDES.contains(&symbol.as_str()))
        .collect::<HashSet<_>>();

    let mut missing = exports
        .iter()
        .filter(|&symbol| !wasm_exports.contains(symbol))
        .map(String::as_str)
        .collect::<Vec<_>>();
    missing.sort_unstable();

    if !missing.is_empty() {
        Err(anyhow!(format!(
            "Unmatched wasm exports:\n{}",
            missing.join("\n")
        )))?;
    }

    Ok(())
}
