use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
};

use anyhow::Result;
use regex::{Regex, RegexBuilder};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod build_tables;
mod dedup;
mod grammar_files;
mod grammars;
mod nfa;
mod node_types;
pub mod parse_grammar;
mod prepare_grammar;
mod render;
mod rules;
mod tables;

use build_tables::build_tables;
pub use build_tables::ParseTableBuilderError;
use grammars::InputGrammar;
pub use node_types::{SuperTypeCycleError, VariableInfoError};
use parse_grammar::parse_grammar;
pub use parse_grammar::ParseGrammarError;
use prepare_grammar::prepare_grammar;
pub use prepare_grammar::PrepareGrammarError;
use render::render_c_code;
pub use render::{ABI_VERSION_MAX, ABI_VERSION_MIN};

static JSON_COMMENT_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    RegexBuilder::new("^\\s*//.*")
        .multi_line(true)
        .build()
        .unwrap()
});

struct GeneratedParser {
    c_code: String,
    node_types_json: String,
}

pub const ALLOC_HEADER: &str = include_str!("templates/alloc.h");
pub const ARRAY_HEADER: &str = include_str!("templates/array.h");

pub type GenerateResult<T> = Result<T, GenerateError>;

#[derive(Debug, Error, Serialize)]
pub enum GenerateError {
    #[error("Error with specified path -- {0}")]
    GrammarPath(String),
    #[error("{0}")]
    IO(String),
    #[error(transparent)]
    LoadGrammarFile(#[from] LoadGrammarError),
    #[error(transparent)]
    ParseGrammar(#[from] ParseGrammarError),
    #[error(transparent)]
    Prepare(#[from] PrepareGrammarError),
    #[error(transparent)]
    VariableInfo(#[from] VariableInfoError),
    #[error(transparent)]
    BuildTables(#[from] ParseTableBuilderError),
    #[error(transparent)]
    ParseVersion(#[from] ParseVersionError),
    #[error(transparent)]
    SuperTypeCycle(#[from] SuperTypeCycleError),
}

impl From<std::io::Error> for GenerateError {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value.to_string())
    }
}

pub type LoadGrammarFileResult<T> = Result<T, LoadGrammarError>;

#[derive(Debug, Error, Serialize)]
pub enum LoadGrammarError {
    #[error("Path to a grammar file with `.js` or `.json` extension is required")]
    InvalidPath,
    #[error("Failed to load grammar.js -- {0}")]
    LoadJSGrammarFile(#[from] JSError),
    #[error("Failed to load grammar.json -- {0}")]
    IO(String),
    #[error("Unknown grammar file extension: {0:?}")]
    FileExtension(PathBuf),
}

impl From<std::io::Error> for LoadGrammarError {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value.to_string())
    }
}

#[derive(Debug, Error, Serialize)]
pub enum ParseVersionError {
    #[error("{0}")]
    Version(String),
    #[error("{0}")]
    JSON(String),
    #[error("{0}")]
    IO(String),
}

pub type JSResult<T> = Result<T, JSError>;

#[derive(Debug, Error, Serialize)]
pub enum JSError {
    #[error("Failed to run `{runtime}` -- {error}")]
    JSRuntimeSpawn { runtime: String, error: String },
    #[error("Got invalid UTF8 from `{runtime}` -- {error}")]
    JSRuntimeUtf8 { runtime: String, error: String },
    #[error("`{runtime}` process exited with status {code}")]
    JSRuntimeExit { runtime: String, code: i32 },
    #[error("{0}")]
    IO(String),
    #[error("Could not parse this package's version as semver -- {0}")]
    Semver(String),
    #[error("Failed to serialze grammar JSON -- {0}")]
    Serialzation(String),
}

impl From<std::io::Error> for JSError {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value.to_string())
    }
}

impl From<serde_json::Error> for JSError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialzation(value.to_string())
    }
}

impl From<semver::Error> for JSError {
    fn from(value: semver::Error) -> Self {
        Self::Semver(value.to_string())
    }
}

pub fn generate_parser_in_directory(
    repo_path: &Path,
    out_path: Option<&str>,
    grammar_path: Option<&str>,
    mut abi_version: usize,
    report_symbol_name: Option<&str>,
    js_runtime: Option<&str>,
) -> GenerateResult<()> {
    let mut repo_path = repo_path.to_owned();
    let mut grammar_path = grammar_path;

    // Populate a new empty grammar directory.
    if let Some(path) = grammar_path {
        let path = PathBuf::from(path);
        if !path
            .try_exists()
            .map_err(|e| GenerateError::GrammarPath(e.to_string()))?
        {
            fs::create_dir_all(&path)?;
            grammar_path = None;
            repo_path = path;
        }
    }

    let grammar_path = grammar_path.map_or_else(|| repo_path.join("grammar.js"), PathBuf::from);

    // Read the grammar file.
    let grammar_json = load_grammar_file(&grammar_path, js_runtime)?;

    let src_path = out_path.map_or_else(|| repo_path.join("src"), PathBuf::from);
    let header_path = src_path.join("tree_sitter");

    // Ensure that the output directories exist.
    fs::create_dir_all(&src_path)?;
    fs::create_dir_all(&header_path)?;

    if grammar_path.file_name().unwrap() != "grammar.json" {
        fs::write(src_path.join("grammar.json"), &grammar_json).map_err(|e| {
            GenerateError::IO(format!(
                "Failed to write grammar.json to {} -- {e}",
                src_path.display()
            ))
        })?;
    }

    // Parse and preprocess the grammar.
    let input_grammar = parse_grammar(&grammar_json)?;

    let semantic_version = read_grammar_version(&repo_path)?;

    if semantic_version.is_none() && abi_version > ABI_VERSION_MIN {
        println!("Warning: No `tree-sitter.json` file found in your grammar, this file is required to generate with ABI {abi_version}. Using ABI version {ABI_VERSION_MIN} instead.");
        println!("This file can be set up with `tree-sitter init`. For more information, see https://tree-sitter.github.io/tree-sitter/cli/init.");
        abi_version = ABI_VERSION_MIN;
    }

    // Generate the parser and related files.
    let GeneratedParser {
        c_code,
        node_types_json,
    } = generate_parser_for_grammar_with_opts(
        &input_grammar,
        abi_version,
        semantic_version.map(|v| (v.major as u8, v.minor as u8, v.patch as u8)),
        report_symbol_name,
    )?;

    write_file(&src_path.join("parser.c"), c_code)?;
    write_file(&src_path.join("node-types.json"), node_types_json)?;
    write_file(&header_path.join("alloc.h"), ALLOC_HEADER)?;
    write_file(&header_path.join("array.h"), ARRAY_HEADER)?;
    write_file(&header_path.join("parser.h"), tree_sitter::PARSER_HEADER)?;

    Ok(())
}

pub fn generate_parser_for_grammar(
    grammar_json: &str,
    semantic_version: Option<(u8, u8, u8)>,
) -> GenerateResult<(String, String)> {
    let grammar_json = JSON_COMMENT_REGEX.replace_all(grammar_json, "\n");
    let input_grammar = parse_grammar(&grammar_json)?;
    let parser = generate_parser_for_grammar_with_opts(
        &input_grammar,
        tree_sitter::LANGUAGE_VERSION,
        semantic_version,
        None,
    )?;
    Ok((input_grammar.name, parser.c_code))
}

fn generate_parser_for_grammar_with_opts(
    input_grammar: &InputGrammar,
    abi_version: usize,
    semantic_version: Option<(u8, u8, u8)>,
    report_symbol_name: Option<&str>,
) -> GenerateResult<GeneratedParser> {
    let (syntax_grammar, lexical_grammar, inlines, simple_aliases) =
        prepare_grammar(input_grammar)?;
    let variable_info =
        node_types::get_variable_info(&syntax_grammar, &lexical_grammar, &simple_aliases)?;
    let node_types_json = node_types::generate_node_types_json(
        &syntax_grammar,
        &lexical_grammar,
        &simple_aliases,
        &variable_info,
    )?;
    let supertype_symbol_map =
        node_types::get_supertype_symbol_map(&syntax_grammar, &simple_aliases, &variable_info);
    let tables = build_tables(
        &syntax_grammar,
        &lexical_grammar,
        &simple_aliases,
        &variable_info,
        &inlines,
        report_symbol_name,
    )?;
    let c_code = render_c_code(
        &input_grammar.name,
        tables,
        syntax_grammar,
        lexical_grammar,
        simple_aliases,
        abi_version,
        semantic_version,
        supertype_symbol_map,
    );
    Ok(GeneratedParser {
        c_code,
        node_types_json: serde_json::to_string_pretty(&node_types_json).unwrap(),
    })
}

/// This will read the `tree-sitter.json` config file and attempt to extract the version.
///
/// If the file is not found in the current directory or any of its parent directories, this will
/// return `None` to maintain backwards compatibility. If the file is found but the version cannot
/// be parsed as semver, this will return an error.
fn read_grammar_version(repo_path: &Path) -> Result<Option<Version>, ParseVersionError> {
    #[derive(Deserialize)]
    struct TreeSitterJson {
        metadata: Metadata,
    }

    #[derive(Deserialize)]
    struct Metadata {
        version: String,
    }

    let filename = "tree-sitter.json";
    let mut path = repo_path.join(filename);

    loop {
        let json = path
            .exists()
            .then(|| {
                let contents = fs::read_to_string(path.as_path()).map_err(|e| {
                    ParseVersionError::IO(format!("Failed to read `{}` -- {e}", path.display()))
                })?;
                serde_json::from_str::<TreeSitterJson>(&contents).map_err(|e| {
                    ParseVersionError::JSON(format!("Failed to parse `{}` -- {e}", path.display()))
                })
            })
            .transpose()?;
        if let Some(json) = json {
            return Version::parse(&json.metadata.version)
                .map_err(|e| {
                    ParseVersionError::Version(format!(
                        "Failed to parse `{}` version as semver -- {e}",
                        path.display()
                    ))
                })
                .map(Some);
        }
        path.pop(); // filename
        if !path.pop() {
            return Ok(None);
        }
        path.push(filename);
    }
}

pub fn load_grammar_file(
    grammar_path: &Path,
    js_runtime: Option<&str>,
) -> LoadGrammarFileResult<String> {
    if grammar_path.is_dir() {
        Err(LoadGrammarError::InvalidPath)?;
    }
    match grammar_path.extension().and_then(|e| e.to_str()) {
        Some("js") => Ok(load_js_grammar_file(grammar_path, js_runtime)?),
        Some("json") => Ok(fs::read_to_string(grammar_path)?),
        _ => Err(LoadGrammarError::FileExtension(grammar_path.to_owned()))?,
    }
}

fn load_js_grammar_file(grammar_path: &Path, js_runtime: Option<&str>) -> JSResult<String> {
    let grammar_path = fs::canonicalize(grammar_path)?;

    #[cfg(windows)]
    let grammar_path = url::Url::from_file_path(grammar_path)
        .expect("Failed to convert path to URL")
        .to_string();

    let js_runtime = js_runtime.unwrap_or("node");

    let mut js_command = Command::new(js_runtime);
    match js_runtime {
        "node" => {
            js_command.args(["--input-type=module", "-"]);
        }
        "bun" => {
            js_command.arg("-");
        }
        "deno" => {
            js_command.args(["run", "--allow-all", "-"]);
        }
        _ => {}
    }

    let mut js_process = js_command
        .env("TREE_SITTER_GRAMMAR_PATH", grammar_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| JSError::JSRuntimeSpawn {
            runtime: js_runtime.to_string(),
            error: e.to_string(),
        })?;

    let mut js_stdin = js_process
        .stdin
        .take()
        .ok_or_else(|| JSError::IO(format!("Failed to open stdin for `{js_runtime}`")))?;

    let cli_version = Version::parse(env!("CARGO_PKG_VERSION"))?;
    write!(
        js_stdin,
        "globalThis.TREE_SITTER_CLI_VERSION_MAJOR = {};
         globalThis.TREE_SITTER_CLI_VERSION_MINOR = {};
         globalThis.TREE_SITTER_CLI_VERSION_PATCH = {};",
        cli_version.major, cli_version.minor, cli_version.patch,
    )
    .map_err(|e| {
        JSError::IO(format!(
            "Failed to write tree-sitter version to `{js_runtime}`'s stdin -- {e}"
        ))
    })?;
    js_stdin.write(include_bytes!("./dsl.js")).map_err(|e| {
        JSError::IO(format!(
            "Failed to write grammar dsl to `{js_runtime}`'s stdin -- {e}"
        ))
    })?;
    drop(js_stdin);

    let output = js_process
        .wait_with_output()
        .map_err(|e| JSError::IO(format!("Failed to read output from `{js_runtime}` -- {e}")))?;
    match output.status.code() {
        None => panic!("`{js_runtime}` process was killed"),
        Some(0) => {
            let stdout = String::from_utf8(output.stdout).map_err(|e| JSError::JSRuntimeUtf8 {
                runtime: js_runtime.to_string(),
                error: e.to_string(),
            })?;

            let mut grammar_json = &stdout[..];

            if let Some(pos) = stdout.rfind('\n') {
                // If there's a newline, split the last line from the rest of the output
                let node_output = &stdout[..pos];
                grammar_json = &stdout[pos + 1..];

                let mut stdout = std::io::stdout().lock();
                stdout.write_all(node_output.as_bytes())?;
                stdout.write_all(b"\n")?;
                stdout.flush()?;
            }

            Ok(serde_json::to_string_pretty(&serde_json::from_str::<
                serde_json::Value,
            >(grammar_json)?)?)
        }
        Some(code) => Err(JSError::JSRuntimeExit {
            runtime: js_runtime.to_string(),
            code,
        }),
    }
}

pub fn write_file(path: &Path, body: impl AsRef<[u8]>) -> GenerateResult<()> {
    fs::write(path, body)
        .map_err(|e| GenerateError::IO(format!("Failed to write {:?} -- {e}", path.file_name())))
}
