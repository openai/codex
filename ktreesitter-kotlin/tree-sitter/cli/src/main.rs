use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use anstyle::{AnsiColor, Color, Style};
use anyhow::{anyhow, Context, Result};
use clap::{crate_authors, Args, Command, FromArgMatches as _, Subcommand, ValueEnum};
use clap_complete::generate;
use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input};
use heck::ToUpperCamelCase;
use regex::Regex;
use semver::Version as SemverVersion;
use tree_sitter::{ffi, Parser, Point};
use tree_sitter_cli::{
    fuzz::{
        fuzz_language_corpus, FuzzOptions, EDIT_COUNT, ITERATION_COUNT, LOG_ENABLED,
        LOG_GRAPH_ENABLED, START_SEED,
    },
    highlight::{self, HighlightOptions},
    init::{generate_grammar_files, get_root_path, JsonConfigOpts},
    input::{get_input, get_tmp_source_file, CliInput},
    logger,
    parse::{self, ParseDebugType, ParseFileOptions, ParseOutput, ParseTheme},
    playground, query,
    tags::{self, TagsOptions},
    test::{self, TestOptions, TestStats},
    test_highlight, test_tags, util, version, wasm,
};
use tree_sitter_config::Config;
use tree_sitter_highlight::Highlighter;
use tree_sitter_loader::{self as loader, TreeSitterJSON};
use tree_sitter_tags::TagsContext;
use url::Url;

const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_SHA: Option<&'static str> = option_env!("BUILD_SHA");
const DEFAULT_GENERATE_ABI_VERSION: usize = 15;

#[derive(Subcommand)]
#[command(about="Generates and tests parsers", author=crate_authors!("\n"), styles=get_styles())]
enum Commands {
    /// Generate a default config file
    InitConfig(InitConfig),
    /// Initialize a grammar repository
    Init(Init),
    /// Generate a parser
    Generate(Generate),
    /// Compile a parser
    Build(Build),
    /// Parse files
    Parse(Parse),
    /// Run a parser's tests
    Test(Test),
    /// Increment the version of a grammar
    Version(Version),
    /// Fuzz a parser
    Fuzz(Fuzz),
    /// Search files using a syntax tree query
    Query(Query),
    /// Highlight a file
    Highlight(Highlight),
    /// Generate a list of tags
    Tags(Tags),
    /// Start local playground for a parser in the browser
    Playground(Playground),
    /// Print info about all known language parsers
    DumpLanguages(DumpLanguages),
    /// Generate shell completions
    Complete(Complete),
}

#[derive(Args)]
struct InitConfig;

#[derive(Args)]
#[command(alias = "i")]
struct Init {
    /// Update outdated files
    #[arg(long, short)]
    pub update: bool,
}

#[derive(Args)]
#[command(alias = "gen", alias = "g")]
struct Generate {
    /// The path to the grammar file
    #[arg(index = 1)]
    pub grammar_path: Option<String>,
    /// Show debug log during generation
    #[arg(long, short)]
    pub log: bool,
    /// Deprecated (no-op)
    #[arg(long)]
    pub no_bindings: bool,
    #[arg(
        long = "abi",
        value_name = "VERSION",
        env = "TREE_SITTER_ABI_VERSION",
        help = format!(concat!(
                    "Select the language ABI version to generate (default {}).\n",
                    "Use --abi=latest to generate the newest supported version ({}).",
                    ),
                DEFAULT_GENERATE_ABI_VERSION,
                tree_sitter::LANGUAGE_VERSION,
                )
    )]
    pub abi_version: Option<String>,
    /// Compile all defined languages in the current dir
    #[arg(long, short = 'b')]
    pub build: bool,
    /// Compile a parser in debug mode
    #[arg(long, short = '0')]
    pub debug_build: bool,
    /// The path to the directory containing the parser library
    #[arg(long, value_name = "PATH")]
    pub libdir: Option<String>,
    /// The path to output the generated source files
    #[arg(long, short, value_name = "DIRECTORY")]
    pub output: Option<String>,
    /// Produce a report of the states for the given rule, use `-` to report every rule
    #[arg(long)]
    pub report_states_for_rule: Option<String>,
    /// Report conflicts in a JSON format
    #[arg(long)]
    pub json: bool,
    /// The name or path of the JavaScript runtime to use for generating parsers
    #[arg(
        long,
        value_name = "EXECUTABLE",
        env = "TREE_SITTER_JS_RUNTIME",
        default_value = "node"
    )]
    pub js_runtime: Option<String>,
}

#[derive(Args)]
#[command(alias = "b")]
struct Build {
    /// Build a WASM module instead of a dynamic library
    #[arg(short, long)]
    pub wasm: bool,
    /// Run emscripten via docker even if it is installed locally (only if building a WASM module
    /// with --wasm)
    #[arg(short, long)]
    pub docker: bool,
    /// The path to output the compiled file
    #[arg(short, long)]
    pub output: Option<String>,
    /// The path to the grammar directory
    #[arg(index = 1, num_args = 1)]
    pub path: Option<String>,
    /// Make the parser reuse the same allocator as the library
    #[arg(long)]
    pub reuse_allocator: bool,
    /// Compile a parser in debug mode
    #[arg(long, short = '0')]
    pub debug: bool,
}

#[derive(Args)]
#[command(alias = "p")]
struct Parse {
    /// The path to a file with paths to source file(s)
    #[arg(long = "paths")]
    pub paths_file: Option<PathBuf>,
    /// The source file(s) to use
    #[arg(num_args=1..)]
    pub paths: Option<Vec<PathBuf>>,
    /// Select a language by the scope instead of a file extension
    #[arg(long)]
    pub scope: Option<String>,
    /// Show parsing debug log
    #[arg(long, short = 'd')] // TODO: Rework once clap adds `default_missing_value_t`
    #[allow(clippy::option_option)]
    pub debug: Option<Option<ParseDebugType>>,
    /// Compile a parser in debug mode
    #[arg(long, short = '0')]
    pub debug_build: bool,
    /// Produce the log.html file with debug graphs
    #[arg(long, short = 'D')]
    pub debug_graph: bool,
    /// Compile parsers to wasm instead of native dynamic libraries
    #[arg(long)]
    pub wasm: bool,
    /// Output the parse data with graphviz dot
    #[arg(long = "dot")]
    pub output_dot: bool,
    /// Output the parse data in XML format
    #[arg(long = "xml", short = 'x')]
    pub output_xml: bool,
    /// Output the parse data in a pretty-printed CST format
    #[arg(long = "cst", short = 'c')]
    pub output_cst: bool,
    /// Show parsing statistic
    #[arg(long, short)]
    pub stat: bool,
    /// Interrupt the parsing process by timeout (µs)
    #[arg(long)]
    pub timeout: Option<u64>,
    /// Measure execution time
    #[arg(long, short)]
    pub time: bool,
    /// Suppress main output
    #[arg(long, short)]
    pub quiet: bool,
    #[allow(clippy::doc_markdown)]
    /// Apply edits in the format: \"row,col|position delcount insert_text\", can be supplied
    /// multiple times
    #[arg(
        long,
        num_args = 1..,
    )]
    pub edits: Option<Vec<String>>,
    /// The encoding of the input files
    #[arg(long)]
    pub encoding: Option<Encoding>,
    /// Open `log.html` in the default browser, if `--debug-graph` is supplied
    #[arg(long)]
    pub open_log: bool,
    /// Output parsing results in a JSON format
    #[arg(long, short = 'j')]
    pub json: bool,
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
    /// Parse the contents of a specific test
    #[arg(long, short = 'n')]
    #[clap(conflicts_with = "paths", conflicts_with = "paths_file")]
    pub test_number: Option<u32>,
    /// Force rebuild the parser
    #[arg(short, long)]
    pub rebuild: bool,
    /// Omit ranges in the output
    #[arg(long)]
    pub no_ranges: bool,
}

#[derive(ValueEnum, Clone)]
pub enum Encoding {
    Utf8,
    Utf16LE,
    Utf16BE,
}

#[derive(Args)]
#[command(alias = "t")]
struct Test {
    /// Only run corpus test cases whose name matches the given regex
    #[arg(long, short)]
    pub include: Option<Regex>,
    /// Only run corpus test cases whose name does not match the given regex
    #[arg(long, short)]
    pub exclude: Option<Regex>,
    /// Only run corpus test cases from from a given filename
    #[arg(long)]
    pub file_name: Option<String>,
    /// Update all syntax trees in corpus files with current parser output
    #[arg(long, short)]
    pub update: bool,
    /// Show parsing debug log
    #[arg(long, short = 'd')]
    pub debug: bool,
    /// Compile a parser in debug mode
    #[arg(long, short = '0')]
    pub debug_build: bool,
    /// Produce the log.html file with debug graphs
    #[arg(long, short = 'D')]
    pub debug_graph: bool,
    /// Compile parsers to wasm instead of native dynamic libraries
    #[arg(long)]
    pub wasm: bool,
    /// Open `log.html` in the default browser, if `--debug-graph` is supplied
    #[arg(long)]
    pub open_log: bool,
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
    /// Force showing fields in test diffs
    #[arg(long)]
    pub show_fields: bool,
    /// Show parsing statistics
    #[arg(long)]
    pub stat: Option<TestStats>,
    /// Force rebuild the parser
    #[arg(short, long)]
    pub rebuild: bool,
    /// Show only the pass-fail overview tree
    #[arg(long)]
    pub overview_only: bool,
}

#[derive(Args)]
#[command(alias = "publish")]
/// Increment the version of a grammar
struct Version {
    #[arg(num_args = 1)]
    /// The version to bump to
    pub version: SemverVersion,
}

#[derive(Args)]
#[command(alias = "f")]
struct Fuzz {
    /// List of test names to skip
    #[arg(long, short)]
    pub skip: Option<Vec<String>>,
    /// Subdirectory to the language
    #[arg(long)]
    pub subdir: Option<String>,
    /// Maximum number of edits to perform per fuzz test
    #[arg(long)]
    pub edits: Option<usize>,
    /// Number of fuzzing iterations to run per test
    #[arg(long)]
    pub iterations: Option<usize>,
    /// Only fuzz corpus test cases whose name matches the given regex
    #[arg(long, short)]
    pub include: Option<Regex>,
    /// Only fuzz corpus test cases whose name does not match the given regex
    #[arg(long, short)]
    pub exclude: Option<Regex>,
    /// Enable logging of graphs and input
    #[arg(long)]
    pub log_graphs: bool,
    /// Enable parser logging
    #[arg(long, short)]
    pub log: bool,
    /// Force rebuild the parser
    #[arg(short, long)]
    pub rebuild: bool,
}

#[derive(Args)]
#[command(alias = "q")]
struct Query {
    /// Path to a file with queries
    #[arg(index = 1, required = true)]
    query_path: PathBuf,
    /// Measure execution time
    #[arg(long, short)]
    pub time: bool,
    /// Suppress main output
    #[arg(long, short)]
    pub quiet: bool,
    /// The path to a file with paths to source file(s)
    #[arg(long = "paths")]
    pub paths_file: Option<PathBuf>,
    /// The source file(s) to use
    #[arg(index = 2, num_args=1..)]
    pub paths: Option<Vec<PathBuf>>,
    /// The range of byte offsets in which the query will be executed
    #[arg(long)]
    pub byte_range: Option<String>,
    /// The range of rows in which the query will be executed
    #[arg(long)]
    pub row_range: Option<String>,
    /// Select a language by the scope instead of a file extension
    #[arg(long)]
    pub scope: Option<String>,
    /// Order by captures instead of matches
    #[arg(long, short)]
    pub captures: bool,
    /// Whether to run query tests or not
    #[arg(long)]
    pub test: bool,
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
    /// Query the contents of a specific test
    #[arg(long, short = 'n')]
    #[clap(conflicts_with = "paths", conflicts_with = "paths_file")]
    pub test_number: Option<u32>,
}

#[derive(Args)]
#[command(alias = "hi")]
struct Highlight {
    /// Generate highlighting as an HTML document
    #[arg(long, short = 'H')]
    pub html: bool,
    /// When generating HTML, use css classes rather than inline styles
    #[arg(long)]
    pub css_classes: bool,
    /// Check that highlighting captures conform strictly to standards
    #[arg(long)]
    pub check: bool,
    /// The path to a file with captures
    #[arg(long)]
    pub captures_path: Option<PathBuf>,
    /// The paths to files with queries
    #[arg(long, num_args = 1..)]
    pub query_paths: Option<Vec<PathBuf>>,
    /// Select a language by the scope instead of a file extension
    #[arg(long)]
    pub scope: Option<String>,
    /// Measure execution time
    #[arg(long, short)]
    pub time: bool,
    /// Suppress main output
    #[arg(long, short)]
    pub quiet: bool,
    /// The path to a file with paths to source file(s)
    #[arg(long = "paths")]
    pub paths_file: Option<PathBuf>,
    /// The source file(s) to use
    #[arg(num_args = 1..)]
    pub paths: Option<Vec<PathBuf>>,
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
    /// Highlight the contents of a specific test
    #[arg(long, short = 'n')]
    #[clap(conflicts_with = "paths", conflicts_with = "paths_file")]
    pub test_number: Option<u32>,
}

#[derive(Args)]
struct Tags {
    /// Select a language by the scope instead of a file extension
    #[arg(long)]
    pub scope: Option<String>,
    /// Measure execution time
    #[arg(long, short)]
    pub time: bool,
    /// Suppress main output
    #[arg(long, short)]
    pub quiet: bool,
    /// The path to a file with paths to source file(s)
    #[arg(long = "paths")]
    pub paths_file: Option<PathBuf>,
    /// The source file(s) to use
    #[arg(num_args = 1..)]
    pub paths: Option<Vec<PathBuf>>,
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
    /// Generate tags from the contents of a specific test
    #[arg(long, short = 'n')]
    #[clap(conflicts_with = "paths", conflicts_with = "paths_file")]
    pub test_number: Option<u32>,
}

#[derive(Args)]
#[command(alias = "play", alias = "pg", alias = "web-ui")]
struct Playground {
    /// Don't open in default browser
    #[arg(long, short)]
    pub quiet: bool,
    /// Path to the directory containing the grammar and wasm files
    #[arg(long)]
    pub grammar_path: Option<String>,
}

#[derive(Args)]
#[command(alias = "langs")]
struct DumpLanguages {
    /// The path to an alternative config.json file
    #[arg(long)]
    pub config_path: Option<PathBuf>,
}

#[derive(Args)]
#[command(alias = "comp")]
struct Complete {
    /// The shell to generate completions for
    #[arg(long, short, value_enum)]
    pub shell: Shell,
}

#[derive(ValueEnum, Clone)]
pub enum Shell {
    Bash,
    Elvish,
    Fish,
    PowerShell,
    Zsh,
    Nushell,
}

impl InitConfig {
    fn run() -> Result<()> {
        if let Ok(Some(config_path)) = Config::find_config_file() {
            return Err(anyhow!(
                "Remove your existing config file first: {}",
                config_path.to_string_lossy()
            ));
        }
        let mut config = Config::initial()?;
        config.add(tree_sitter_loader::Config::initial())?;
        config.add(tree_sitter_cli::highlight::ThemeConfig::default())?;
        config.save()?;
        println!(
            "Saved initial configuration to {}",
            config.location.display()
        );
        Ok(())
    }
}

impl Init {
    fn run(self, current_dir: &Path) -> Result<()> {
        let configure_json = !current_dir.join("tree-sitter.json").exists();

        let (language_name, json_config_opts) = if configure_json {
            let mut opts = JsonConfigOpts::default();

            let name = || {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Parser name")
                    .validate_with(|input: &String| {
                        if input.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                            Ok(())
                        } else {
                            Err("The name must be lowercase and contain only letters, digits, and underscores")
                        }
                    })
                    .interact_text()
            };

            let camelcase_name = |name: &str| {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("CamelCase name")
                    .default(name.to_upper_camel_case())
                    .validate_with(|input: &String| {
                        if input
                            .chars()
                            .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit() || c == '_')
                        {
                            Ok(())
                        } else {
                            Err("The name must contain only letters, digits, and underscores")
                        }
                    })
                    .interact_text()
            };

            let title = |name: &str| {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Title (human-readable name)")
                    .default(name.to_upper_camel_case())
                    .interact_text()
            };

            let description = |name: &str| {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Description")
                    .default(format!(
                        "{} grammar for tree-sitter",
                        name.to_upper_camel_case()
                    ))
                    .show_default(false)
                    .allow_empty(true)
                    .interact_text()
            };

            let repository = |name: &str| {
                Input::<Url>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Repository URL")
                    .allow_empty(true)
                    .default(
                        Url::parse(&format!(
                            "https://github.com/tree-sitter/tree-sitter-{name}"
                        ))
                        .expect("Failed to parse default repository URL"),
                    )
                    .show_default(false)
                    .interact_text()
            };

            let funding = || {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Funding URL")
                    .allow_empty(true)
                    .validate_with(|input: &String| {
                        if input.trim().is_empty()
                            || Url::parse(input)
                                .is_ok_and(|u| u.scheme() == "http" || u.scheme() == "https")
                        {
                            Ok(())
                        } else {
                            Err("The URL must start with 'http://' or 'https://'")
                        }
                    })
                    .interact_text()
                    .map(|e| (!e.trim().is_empty()).then(|| Url::parse(&e).unwrap()))
            };

            let scope = |name: &str| {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("TextMate scope")
                    .default(format!("source.{name}"))
                    .validate_with(|input: &String| {
                        if input.starts_with("source.") || input.starts_with("text.") {
                            Ok(())
                        } else {
                            Err("The scope must start with 'source.' or 'text.'")
                        }
                    })
                    .interact_text()
            };

            let file_types = |name: &str| {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("File types (space-separated)")
                    .default(name.to_string())
                    .interact_text()
                    .map(|ft| {
                        let mut set = HashSet::new();
                        for ext in ft.split(' ') {
                            let ext = ext.trim();
                            if !ext.is_empty() {
                                set.insert(ext.to_string());
                            }
                        }
                        set.into_iter().collect::<Vec<_>>()
                    })
            };

            let initial_version = || {
                Input::<SemverVersion>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Version")
                    .default(SemverVersion::new(0, 1, 0))
                    .interact_text()
            };

            let license = || {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("License")
                    .default("MIT".to_string())
                    .allow_empty(true)
                    .interact()
            };

            let author = || {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Author name")
                    .interact_text()
            };

            let email = || {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Author email")
                    .allow_empty(true)
                    .interact_text()
                    .map(|e| (!e.trim().is_empty()).then_some(e))
            };

            let url = || {
                Input::<String>::with_theme(&ColorfulTheme::default())
                    .with_prompt("Author URL")
                    .allow_empty(true)
                    .validate_with(|input: &String| -> Result<(), &str> {
                        if input.trim().is_empty() || Url::parse(input).is_ok() {
                            Ok(())
                        } else {
                            Err("This is not a valid URL")
                        }
                    })
                    .interact_text()
                    .map(|e| (!e.trim().is_empty()).then(|| Url::parse(&e).unwrap()))
            };

            let choices = [
                "name",
                "camelcase",
                "title",
                "description",
                "repository",
                "funding",
                "scope",
                "file_types",
                "version",
                "license",
                "author",
                "email",
                "url",
                "exit",
            ];

            macro_rules! set_choice {
                ($choice:expr) => {
                    match $choice {
                        "name" => opts.name = name()?,
                        "camelcase" => opts.camelcase = camelcase_name(&opts.name)?,
                        "title" => opts.title = title(&opts.name)?,
                        "description" => opts.description = description(&opts.name)?,
                        "repository" => opts.repository = Some(repository(&opts.name)?),
                        "funding" => opts.funding = funding()?,
                        "scope" => opts.scope = scope(&opts.name)?,
                        "file_types" => opts.file_types = file_types(&opts.name)?,
                        "version" => opts.version = initial_version()?,
                        "license" => opts.license = license()?,
                        "author" => opts.author = author()?,
                        "email" => opts.email = email()?,
                        "url" => opts.url = url()?,
                        "exit" => break,
                        _ => unreachable!(),
                    }
                };
            }

            // Initial configuration
            for choice in choices.iter().take(choices.len() - 1) {
                set_choice!(*choice);
            }

            // Loop for editing the configuration
            loop {
                println!(
                    "Your current configuration:\n{}",
                    serde_json::to_string_pretty(&opts)?
                );

                if Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Does the config above look correct?")
                    .interact()?
                {
                    break;
                }

                let idx = FuzzySelect::with_theme(&ColorfulTheme::default())
                    .with_prompt("Which field would you like to change?")
                    .items(&choices)
                    .interact()?;

                set_choice!(choices[idx]);
            }

            (opts.name.clone(), Some(opts))
        } else {
            let mut json = serde_json::from_str::<TreeSitterJSON>(
                &fs::read_to_string(current_dir.join("tree-sitter.json"))
                    .with_context(|| "Failed to read tree-sitter.json")?,
            )?;
            (json.grammars.swap_remove(0).name, None)
        };

        generate_grammar_files(
            current_dir,
            &language_name,
            self.update,
            json_config_opts.as_ref(),
        )?;

        Ok(())
    }
}

impl Generate {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        if self.no_bindings {
            eprint!("The --no-bindings flag is no longer used and will be removed in v0.25.0");
        }
        if self.log {
            logger::init();
        }
        let abi_version =
            self.abi_version
                .as_ref()
                .map_or(DEFAULT_GENERATE_ABI_VERSION, |version| {
                    if version == "latest" {
                        tree_sitter::LANGUAGE_VERSION
                    } else {
                        version.parse().expect("invalid abi version flag")
                    }
                });
        if let Err(err) = tree_sitter_generate::generate_parser_in_directory(
            current_dir,
            self.output.as_deref(),
            self.grammar_path.as_deref(),
            abi_version,
            self.report_states_for_rule.as_deref(),
            self.js_runtime.as_deref(),
        ) {
            if self.json {
                eprintln!("{}", serde_json::to_string_pretty(&err)?);
                // Exit early to prevent errors from being printed a second time in the caller
                std::process::exit(1);
            } else {
                // Removes extra context associated with the error
                Err(anyhow!(err.to_string())).with_context(|| "Error when generating parser")?;
            }
        }
        if self.build {
            if let Some(path) = self.libdir {
                loader = loader::Loader::with_parser_lib_path(PathBuf::from(path));
            }
            loader.debug_build(self.debug_build);
            loader.languages_at_path(current_dir)?;
        }
        Ok(())
    }
}

impl Build {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        let grammar_path = current_dir.join(self.path.as_deref().unwrap_or_default());

        if self.wasm {
            let output_path = self.output.map(|path| current_dir.join(path));
            let root_path = get_root_path(&grammar_path.join("tree-sitter.json"))?;
            wasm::compile_language_to_wasm(
                &loader,
                Some(&root_path),
                &grammar_path,
                current_dir,
                output_path,
                self.docker,
            )?;
        } else {
            let output_path = if let Some(ref path) = self.output {
                let path = Path::new(path);
                if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    current_dir.join(path)
                }
            } else {
                let file_name = grammar_path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .strip_prefix("tree-sitter-")
                    .unwrap_or("parser");
                current_dir
                    .join(file_name)
                    .with_extension(env::consts::DLL_EXTENSION)
            };

            let flags: &[&str] = match (self.reuse_allocator, self.debug) {
                (true, true) => &["TREE_SITTER_REUSE_ALLOCATOR", "TREE_SITTER_DEBUG"],
                (true, false) => &["TREE_SITTER_REUSE_ALLOCATOR"],
                (false, true) => &["TREE_SITTER_DEBUG"],
                (false, false) => &[],
            };

            loader.debug_build(self.debug);
            loader.force_rebuild(true);

            let config = Config::load(None)?;
            let loader_config = config.get()?;
            loader.find_all_languages(&loader_config).unwrap();
            loader
                .compile_parser_at_path(&grammar_path, output_path, flags)
                .unwrap();
        }
        Ok(())
    }
}

impl Parse {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let color = env::var("NO_COLOR").map_or(true, |v| v != "1");
        let output = if self.output_dot {
            ParseOutput::Dot
        } else if self.output_xml {
            ParseOutput::Xml
        } else if self.output_cst {
            ParseOutput::Cst
        } else if self.quiet || self.json {
            ParseOutput::Quiet
        } else {
            ParseOutput::Normal
        };

        let parse_theme = if color {
            config
                .get::<parse::Config>()
                .with_context(|| "Failed to parse CST theme")?
                .parse_theme
                .unwrap_or_default()
                .into()
        } else {
            ParseTheme::empty()
        };

        let encoding = self.encoding.map(|e| match e {
            Encoding::Utf8 => ffi::TSInputEncodingUTF8,
            Encoding::Utf16LE => ffi::TSInputEncodingUTF16LE,
            Encoding::Utf16BE => ffi::TSInputEncodingUTF16BE,
        });

        let time = self.time;
        let edits = self.edits.unwrap_or_default();
        let cancellation_flag = util::cancel_on_signal();
        let mut parser = Parser::new();

        loader.debug_build(self.debug_build);
        loader.force_rebuild(self.rebuild);

        #[cfg(feature = "wasm")]
        if self.wasm {
            let engine = tree_sitter::wasmtime::Engine::default();
            parser
                .set_wasm_store(tree_sitter::WasmStore::new(&engine).unwrap())
                .unwrap();
            loader.use_wasm(&engine);
        }

        let timeout = self.timeout.unwrap_or_default();

        let mut has_error = false;
        let loader_config = config.get()?;
        loader.find_all_languages(&loader_config)?;

        let should_track_stats = self.stat;
        let mut stats = parse::ParseStats::default();
        let debug: ParseDebugType = match self.debug {
            None => ParseDebugType::Quiet,
            Some(None) => ParseDebugType::Normal,
            Some(Some(specifier)) => specifier,
        };

        let mut options = ParseFileOptions {
            edits: &edits
                .iter()
                .map(std::string::String::as_str)
                .collect::<Vec<&str>>(),
            output,
            print_time: time,
            timeout,
            stats: &mut stats,
            debug,
            debug_graph: self.debug_graph,
            cancellation_flag: Some(&cancellation_flag),
            encoding,
            open_log: self.open_log,
            no_ranges: self.no_ranges,
            parse_theme: &parse_theme,
        };

        let mut update_stats = |stats: &mut parse::ParseStats| {
            let parse_result = stats.parse_summaries.last().unwrap();
            if should_track_stats {
                stats.cumulative_stats.total_parses += 1;
                if parse_result.successful {
                    stats.cumulative_stats.successful_parses += 1;
                }
                if let (Some(duration), Some(bytes)) = (parse_result.duration, parse_result.bytes) {
                    stats.cumulative_stats.total_bytes += bytes;
                    stats.cumulative_stats.total_duration += duration;
                }
            }

            has_error |= !parse_result.successful;
        };

        let input = get_input(
            self.paths_file.as_deref(),
            self.paths,
            self.test_number,
            &cancellation_flag,
        )?;
        match input {
            CliInput::Paths(paths) => {
                let max_path_length = paths
                    .iter()
                    .map(|p| p.to_string_lossy().chars().count())
                    .max()
                    .unwrap_or(0);

                for path in &paths {
                    let path = Path::new(&path);
                    let language = loader
                        .select_language(path, current_dir, self.scope.as_deref())
                        .with_context(|| {
                            anyhow!("Failed to load langauge for path \"{}\"", path.display())
                        })?;

                    parse::parse_file_at_path(
                        &mut parser,
                        &language,
                        path,
                        &path.display().to_string(),
                        max_path_length,
                        &mut options,
                    )?;
                    update_stats(options.stats);
                }
            }

            CliInput::Test {
                name,
                contents,
                languages: language_names,
            } => {
                let path = get_tmp_source_file(&contents)?;
                let languages = loader.languages_at_path(current_dir)?;
                let language = languages
                    .iter()
                    .find(|(_, n)| language_names.contains(&Box::from(n.as_str())))
                    .or_else(|| languages.first())
                    .map(|(l, _)| l.clone())
                    .ok_or_else(|| anyhow!("No language found"))?;

                parse::parse_file_at_path(
                    &mut parser,
                    &language,
                    &path,
                    &name,
                    name.chars().count(),
                    &mut options,
                )?;
                update_stats(&mut stats);
                fs::remove_file(path)?;
            }

            CliInput::Stdin(contents) => {
                // Place user input and parser output on separate lines
                println!();

                let path = get_tmp_source_file(&contents)?;
                let name = "stdin";
                let language = loader.select_language(&path, current_dir, None)?;

                parse::parse_file_at_path(
                    &mut parser,
                    &language,
                    &path,
                    name,
                    name.chars().count(),
                    &mut options,
                )?;
                update_stats(&mut stats);
                fs::remove_file(path)?;
            }
        }

        if should_track_stats {
            println!("\n{}", stats.cumulative_stats);
        }
        if self.json {
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }

        if has_error {
            return Err(anyhow!(""));
        }

        Ok(())
    }
}

impl Test {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let color = env::var("NO_COLOR").map_or(true, |v| v != "1");
        let stat = self.stat.unwrap_or_default();

        loader.debug_build(self.debug_build);
        loader.force_rebuild(self.rebuild);

        let mut parser = Parser::new();

        #[cfg(feature = "wasm")]
        if self.wasm {
            let engine = tree_sitter::wasmtime::Engine::default();
            parser
                .set_wasm_store(tree_sitter::WasmStore::new(&engine).unwrap())
                .unwrap();
            loader.use_wasm(&engine);
        }

        let languages = loader.languages_at_path(current_dir)?;
        let language = &languages
            .first()
            .ok_or_else(|| anyhow!("No language found"))?
            .0;
        parser.set_language(language)?;

        let test_dir = current_dir.join("test");
        let mut stats = parse::Stats::default();

        // Run the corpus tests. Look for them in `test/corpus`.
        let test_corpus_dir = test_dir.join("corpus");
        if test_corpus_dir.is_dir() {
            let mut output = String::new();
            let mut rates = Vec::new();
            let mut opts = TestOptions {
                output: &mut output,
                path: test_corpus_dir,
                debug: self.debug,
                debug_graph: self.debug_graph,
                include: self.include,
                exclude: self.exclude,
                file_name: self.file_name,
                update: self.update,
                open_log: self.open_log,
                languages: languages.iter().map(|(l, n)| (n.as_str(), l)).collect(),
                color,
                test_num: 1,
                parse_rates: &mut rates,
                stat_display: stat,
                stats: &mut stats,
                show_fields: self.show_fields,
                overview_only: self.overview_only,
            };

            test::run_tests_at_path(&mut parser, &mut opts)?;
            println!("\n{stats}");
        }

        // Check that all of the queries are valid.
        test::check_queries_at_path(language, &current_dir.join("queries"))?;

        // Run the syntax highlighting tests.
        let test_highlight_dir = test_dir.join("highlight");
        if test_highlight_dir.is_dir() {
            let mut highlighter = Highlighter::new();
            highlighter.parser = parser;
            test_highlight::test_highlights(
                &loader,
                &config.get()?,
                &mut highlighter,
                &test_highlight_dir,
                color,
            )?;
            parser = highlighter.parser;
        }

        let test_tag_dir = test_dir.join("tags");
        if test_tag_dir.is_dir() {
            let mut tags_context = TagsContext::new();
            tags_context.parser = parser;
            test_tags::test_tags(
                &loader,
                &config.get()?,
                &mut tags_context,
                &test_tag_dir,
                color,
            )?;
        }

        // For the rest of the queries, find their tests and run them
        for entry in walkdir::WalkDir::new(current_dir.join("queries"))
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let stem = entry
                .path()
                .file_stem()
                .map(|s| s.to_str().unwrap_or_default())
                .unwrap_or_default();
            if stem != "highlights" && stem != "tags" {
                let entries = walkdir::WalkDir::new(test_dir.join(stem))
                    .into_iter()
                    .filter_map(|e| {
                        let entry = e.ok()?;
                        if entry.file_type().is_file() {
                            Some(entry)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                if !entries.is_empty() {
                    println!("{stem}:");
                }

                for entry in entries {
                    let path = entry.path();
                    query::query_file_at_path(
                        language,
                        path,
                        &path.display().to_string(),
                        path,
                        false,
                        None,
                        None,
                        true,
                        false,
                        false,
                        false,
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl Version {
    fn run(self, current_dir: PathBuf) -> Result<()> {
        version::Version::new(self.version.to_string(), current_dir).run()
    }
}

impl Fuzz {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        loader.sanitize_build(true);
        loader.force_rebuild(self.rebuild);

        let languages = loader.languages_at_path(current_dir)?;
        let (language, language_name) = &languages
            .first()
            .ok_or_else(|| anyhow!("No language found"))?;

        let mut fuzz_options = FuzzOptions {
            skipped: self.skip,
            subdir: self.subdir,
            edits: self.edits.unwrap_or(*EDIT_COUNT),
            iterations: self.iterations.unwrap_or(*ITERATION_COUNT),
            include: self.include,
            exclude: self.exclude,
            log_graphs: self.log_graphs || *LOG_GRAPH_ENABLED,
            log: self.log || *LOG_ENABLED,
        };

        fuzz_language_corpus(
            language,
            language_name,
            *START_SEED,
            current_dir,
            &mut fuzz_options,
        );
        Ok(())
    }
}

impl Query {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let loader_config = config.get()?;
        loader.find_all_languages(&loader_config)?;
        let query_path = Path::new(&self.query_path);

        let byte_range = self.byte_range.as_ref().and_then(|range| {
            let mut parts = range.split(':');
            let start = parts.next()?.parse().ok()?;
            let end = parts.next().unwrap().parse().ok()?;
            Some(start..end)
        });
        let point_range = self.row_range.as_ref().and_then(|range| {
            let mut parts = range.split(':');
            let start = parts.next()?.parse().ok()?;
            let end = parts.next().unwrap().parse().ok()?;
            Some(Point::new(start, 0)..Point::new(end, 0))
        });

        let cancellation_flag = util::cancel_on_signal();

        let input = get_input(
            self.paths_file.as_deref(),
            self.paths,
            self.test_number,
            &cancellation_flag,
        )?;

        match input {
            CliInput::Paths(paths) => {
                let language = loader.select_language(
                    Path::new(&paths[0]),
                    current_dir,
                    self.scope.as_deref(),
                )?;

                for path in paths {
                    query::query_file_at_path(
                        &language,
                        &path,
                        &path.display().to_string(),
                        query_path,
                        self.captures,
                        byte_range.clone(),
                        point_range.clone(),
                        self.test,
                        self.quiet,
                        self.time,
                        false,
                    )?;
                }
            }
            CliInput::Test {
                name,
                contents,
                languages: language_names,
            } => {
                let path = get_tmp_source_file(&contents)?;
                let languages = loader.languages_at_path(current_dir)?;
                let language = languages
                    .iter()
                    .find(|(_, n)| language_names.contains(&Box::from(n.as_str())))
                    .or_else(|| languages.first())
                    .map(|(l, _)| l.clone())
                    .ok_or_else(|| anyhow!("No language found"))?;
                query::query_file_at_path(
                    &language,
                    &path,
                    &name,
                    query_path,
                    self.captures,
                    byte_range,
                    point_range,
                    self.test,
                    self.quiet,
                    self.time,
                    true,
                )?;
                fs::remove_file(path)?;
            }
            CliInput::Stdin(contents) => {
                // Place user input and query output on separate lines
                println!();

                let path = get_tmp_source_file(&contents)?;
                let language = loader.select_language(&path, current_dir, None)?;
                query::query_file_at_path(
                    &language,
                    &path,
                    "stdin",
                    query_path,
                    self.captures,
                    byte_range,
                    point_range,
                    self.test,
                    self.quiet,
                    self.time,
                    true,
                )?;
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }
}

impl Highlight {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let theme_config: tree_sitter_cli::highlight::ThemeConfig = config.get()?;
        loader.configure_highlights(&theme_config.theme.highlight_names);
        let loader_config = config.get()?;
        loader.find_all_languages(&loader_config)?;

        let cancellation_flag = util::cancel_on_signal();

        let (mut language, mut language_configuration) = (None, None);
        if let Some(scope) = self.scope.as_deref() {
            if let Some((lang, lang_config)) = loader.language_configuration_for_scope(scope)? {
                language = Some(lang);
                language_configuration = Some(lang_config);
            }
            if language.is_none() {
                return Err(anyhow!("Unknown scope '{scope}'"));
            }
        }

        let options = HighlightOptions {
            theme: theme_config.theme,
            check: self.check,
            captures_path: self.captures_path,
            inline_styles: !self.css_classes,
            html: self.html,
            quiet: self.quiet,
            print_time: self.time,
            cancellation_flag: cancellation_flag.clone(),
        };

        let input = get_input(
            self.paths_file.as_deref(),
            self.paths,
            self.test_number,
            &cancellation_flag,
        )?;
        match input {
            CliInput::Paths(paths) => {
                let print_name = paths.len() > 1;
                for path in paths {
                    let (language, language_config) =
                        match (language.clone(), language_configuration) {
                            (Some(l), Some(lc)) => (l, lc),
                            _ => {
                                if let Some((lang, lang_config)) =
                                    loader.language_configuration_for_file_name(&path)?
                                {
                                    (lang, lang_config)
                                } else {
                                    eprintln!(
                                        "{}",
                                        util::lang_not_found_for_path(&path, &loader_config)
                                    );
                                    continue;
                                }
                            }
                        };

                    if let Some(highlight_config) =
                        language_config.highlight_config(language, self.query_paths.as_deref())?
                    {
                        highlight::highlight(
                            &loader,
                            &path,
                            &path.display().to_string(),
                            highlight_config,
                            print_name,
                            &options,
                        )?;
                    } else {
                        eprintln!(
                            "No syntax highlighting config found for path {}",
                            path.display()
                        );
                    }
                }
            }

            CliInput::Test {
                name,
                contents,
                languages: language_names,
            } => {
                let path = get_tmp_source_file(&contents)?;

                let languages = loader.languages_at_path(current_dir)?;
                let language = languages
                    .iter()
                    .find(|(_, n)| language_names.contains(&Box::from(n.as_str())))
                    .or_else(|| languages.first())
                    .map(|(l, _)| l.clone())
                    .ok_or_else(|| anyhow!("No language found in current path"))?;
                let language_config = loader
                    .get_language_configuration_in_current_path()
                    .ok_or_else(|| anyhow!("No language configuration found in current path"))?;

                if let Some(highlight_config) =
                    language_config.highlight_config(language, self.query_paths.as_deref())?
                {
                    highlight::highlight(&loader, &path, &name, highlight_config, false, &options)?;
                } else {
                    eprintln!("No syntax highlighting config found for test {name}");
                }
                fs::remove_file(path)?;
            }

            CliInput::Stdin(contents) => {
                // Place user input and highlight output on separate lines
                println!();

                let path = get_tmp_source_file(&contents)?;

                let (language, language_config) =
                    if let (Some(l), Some(lc)) = (language.clone(), language_configuration) {
                        (l, lc)
                    } else {
                        let languages = loader.languages_at_path(current_dir)?;
                        let language = languages
                            .first()
                            .map(|(l, _)| l.clone())
                            .ok_or_else(|| anyhow!("No language found in current path"))?;
                        let language_configuration = loader
                            .get_language_configuration_in_current_path()
                            .ok_or_else(|| {
                                anyhow!("No language configuration found in current path")
                            })?;
                        (language, language_configuration)
                    };

                if let Some(highlight_config) =
                    language_config.highlight_config(language, self.query_paths.as_deref())?
                {
                    highlight::highlight(
                        &loader,
                        &path,
                        "stdin",
                        highlight_config,
                        false,
                        &options,
                    )?;
                } else {
                    eprintln!(
                        "No syntax highlighting config found for path {}",
                        current_dir.display()
                    );
                }
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }
}

impl Tags {
    fn run(self, mut loader: loader::Loader, current_dir: &Path) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let loader_config = config.get()?;
        loader.find_all_languages(&loader_config)?;

        let cancellation_flag = util::cancel_on_signal();

        let (mut language, mut language_configuration) = (None, None);
        if let Some(scope) = self.scope.as_deref() {
            if let Some((lang, lang_config)) = loader.language_configuration_for_scope(scope)? {
                language = Some(lang);
                language_configuration = Some(lang_config);
            }
            if language.is_none() {
                return Err(anyhow!("Unknown scope '{scope}'"));
            }
        }

        let options = TagsOptions {
            scope: self.scope,
            quiet: self.quiet,
            print_time: self.time,
            cancellation_flag: cancellation_flag.clone(),
        };

        let input = get_input(
            self.paths_file.as_deref(),
            self.paths,
            self.test_number,
            &cancellation_flag,
        )?;
        match input {
            CliInput::Paths(paths) => {
                let indent = paths.len() > 1;
                for path in paths {
                    let (language, language_config) =
                        match (language.clone(), language_configuration) {
                            (Some(l), Some(lc)) => (l, lc),
                            _ => {
                                if let Some((lang, lang_config)) =
                                    loader.language_configuration_for_file_name(&path)?
                                {
                                    (lang, lang_config)
                                } else {
                                    eprintln!(
                                        "{}",
                                        util::lang_not_found_for_path(&path, &loader_config)
                                    );
                                    continue;
                                }
                            }
                        };

                    if let Some(tags_config) = language_config.tags_config(language)? {
                        tags::generate_tags(
                            &path,
                            &path.display().to_string(),
                            tags_config,
                            indent,
                            &options,
                        )?;
                    } else {
                        eprintln!("No tags config found for path {}", path.display());
                    }
                }
            }

            CliInput::Test {
                name,
                contents,
                languages: language_names,
            } => {
                let path = get_tmp_source_file(&contents)?;

                let languages = loader.languages_at_path(current_dir)?;
                let language = languages
                    .iter()
                    .find(|(_, n)| language_names.contains(&Box::from(n.as_str())))
                    .or_else(|| languages.first())
                    .map(|(l, _)| l.clone())
                    .ok_or_else(|| anyhow!("No language found in current path"))?;
                let language_config = loader
                    .get_language_configuration_in_current_path()
                    .ok_or_else(|| anyhow!("No language configuration found in current path"))?;

                if let Some(tags_config) = language_config.tags_config(language)? {
                    tags::generate_tags(&path, &name, tags_config, false, &options)?;
                } else {
                    eprintln!("No tags config found for test {name}");
                }
                fs::remove_file(path)?;
            }

            CliInput::Stdin(contents) => {
                // Place user input and tags output on separate lines
                println!();

                let path = get_tmp_source_file(&contents)?;

                let (language, language_config) =
                    if let (Some(l), Some(lc)) = (language.clone(), language_configuration) {
                        (l, lc)
                    } else {
                        let languages = loader.languages_at_path(current_dir)?;
                        let language = languages
                            .first()
                            .map(|(l, _)| l.clone())
                            .ok_or_else(|| anyhow!("No language found in current path"))?;
                        let language_configuration = loader
                            .get_language_configuration_in_current_path()
                            .ok_or_else(|| {
                                anyhow!("No language configuration found in current path")
                            })?;
                        (language, language_configuration)
                    };

                if let Some(tags_config) = language_config.tags_config(language)? {
                    tags::generate_tags(&path, "stdin", tags_config, false, &options)?;
                } else {
                    eprintln!("No tags config found for path {}", current_dir.display());
                }
                fs::remove_file(path)?;
            }
        }

        Ok(())
    }
}

impl Playground {
    fn run(self, current_dir: &Path) -> Result<()> {
        let open_in_browser = !self.quiet;
        let grammar_path = self.grammar_path.as_deref().map_or(current_dir, Path::new);
        playground::serve(grammar_path, open_in_browser)?;
        Ok(())
    }
}

impl DumpLanguages {
    fn run(self, mut loader: loader::Loader) -> Result<()> {
        let config = Config::load(self.config_path)?;
        let loader_config = config.get()?;
        loader.find_all_languages(&loader_config)?;
        for (configuration, language_path) in loader.get_all_language_configurations() {
            println!(
                concat!(
                    "scope: {}\n",
                    "parser: {:?}\n",
                    "highlights: {:?}\n",
                    "file_types: {:?}\n",
                    "content_regex: {:?}\n",
                    "injection_regex: {:?}\n",
                ),
                configuration.scope.as_ref().unwrap_or(&String::new()),
                language_path,
                configuration.highlights_filenames,
                configuration.file_types,
                configuration.content_regex,
                configuration.injection_regex,
            );
        }
        Ok(())
    }
}

impl Complete {
    fn run(self, cli: &mut Command) {
        let name = cli.get_name().to_string();
        let mut stdout = std::io::stdout();

        match self.shell {
            Shell::Bash => generate(clap_complete::shells::Bash, cli, &name, &mut stdout),
            Shell::Elvish => generate(clap_complete::shells::Elvish, cli, &name, &mut stdout),
            Shell::Fish => generate(clap_complete::shells::Fish, cli, &name, &mut stdout),
            Shell::PowerShell => {
                generate(clap_complete::shells::PowerShell, cli, &name, &mut stdout);
            }
            Shell::Zsh => generate(clap_complete::shells::Zsh, cli, &name, &mut stdout),
            Shell::Nushell => generate(clap_complete_nushell::Nushell, cli, &name, &mut stdout),
        }
    }
}

fn main() {
    let result = run();
    if let Err(err) = &result {
        // Ignore BrokenPipe errors
        if let Some(error) = err.downcast_ref::<std::io::Error>() {
            if error.kind() == std::io::ErrorKind::BrokenPipe {
                return;
            }
        }
        if !err.to_string().is_empty() {
            eprintln!("{err:?}");
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let version = BUILD_SHA.map_or_else(
        || BUILD_VERSION.to_string(),
        |build_sha| format!("{BUILD_VERSION} ({build_sha})"),
    );
    let version: &'static str = Box::leak(version.into_boxed_str());

    let cli = Command::new("tree-sitter")
        .help_template(
            "\
{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
",
        )
        .version(version)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .disable_help_subcommand(true)
        .disable_colored_help(false);
    let mut cli = Commands::augment_subcommands(cli);

    let command = Commands::from_arg_matches(&cli.clone().get_matches())?;

    let current_dir = env::current_dir().unwrap();
    let loader = loader::Loader::new()?;

    match command {
        Commands::InitConfig(_) => InitConfig::run()?,
        Commands::Init(init_options) => init_options.run(&current_dir)?,
        Commands::Generate(generate_options) => generate_options.run(loader, &current_dir)?,
        Commands::Build(build_options) => build_options.run(loader, &current_dir)?,
        Commands::Parse(parse_options) => parse_options.run(loader, &current_dir)?,
        Commands::Test(test_options) => test_options.run(loader, &current_dir)?,
        Commands::Version(version_options) => version_options.run(current_dir)?,
        Commands::Fuzz(fuzz_options) => fuzz_options.run(loader, &current_dir)?,
        Commands::Query(query_options) => query_options.run(loader, &current_dir)?,
        Commands::Highlight(highlight_options) => highlight_options.run(loader, &current_dir)?,
        Commands::Tags(tags_options) => tags_options.run(loader, &current_dir)?,
        Commands::Playground(playground_options) => playground_options.run(&current_dir)?,
        Commands::DumpLanguages(dump_options) => dump_options.run(loader)?,
        Commands::Complete(complete_options) => complete_options.run(&mut cli),
    }

    Ok(())
}

#[must_use]
const fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .header(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
        )
        .literal(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Green))))
        .invalid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .valid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::White))))
}
