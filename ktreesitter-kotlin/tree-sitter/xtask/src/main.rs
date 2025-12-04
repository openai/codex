mod benchmark;
mod build_wasm;
mod bump;
mod check_wasm_exports;
mod clippy;
mod fetch;
mod generate;
mod test;
mod upgrade_emscripten;
mod upgrade_wasmtime;

use std::path::Path;

use anstyle::{AnsiColor, Color, Style};
use anyhow::Result;
use clap::{crate_authors, Args, Command, FromArgMatches as _, Subcommand};
use git2::{Oid, Repository};
use semver::Version;

#[derive(Subcommand)]
#[command(about="Run various tasks", author=crate_authors!("\n"), styles=get_styles())]
enum Commands {
    /// Runs `cargo benchmark` with some optional environment variables set.
    Benchmark(Benchmark),
    /// Compile the Tree-sitter WASM library. This will create two files in the
    /// `lib/binding_web` directory: `tree-sitter.js` and `tree-sitter.wasm`.
    BuildWasm(BuildWasm),
    /// Compile the Tree-sitter WASM standard library.
    BuildWasmStdlib,
    /// Bumps the version of the workspace.
    BumpVersion(BumpVersion),
    /// Checks that WASM exports are synced.
    CheckWasmExports(CheckWasmExports),
    /// Runs `cargo clippy`.
    Clippy(Clippy),
    /// Fetches emscripten.
    FetchEmscripten,
    /// Fetches the fixtures for testing tree-sitter.
    FetchFixtures,
    /// Generate the Rust bindings from the C library.
    GenerateBindings,
    /// Generates the fixtures for testing tree-sitter.
    GenerateFixtures(GenerateFixtures),
    /// Generate the list of exports from Tree-sitter WASM files.
    GenerateWasmExports,
    /// Run the test suite
    Test(Test),
    /// Run the WASM test suite
    TestWasm,
    /// Upgrade the wasmtime dependency.
    UpgradeWasmtime(UpgradeWasmtime),
    /// Upgrade the emscripten file.
    UpgradeEmscripten,
}

#[derive(Args)]
struct Benchmark {
    /// The language to run the benchmarks for.
    #[arg(long, short)]
    language: Option<String>,
    /// The example file to run the benchmarks for.
    #[arg(long, short)]
    example_file_name: Option<String>,
    /// The number of times to parse each sample (default is 5).
    #[arg(long, short, default_value = "5")]
    repetition_count: u32,
    /// Whether to run the benchmarks in debug mode.
    #[arg(long, short = 'g')]
    debug: bool,
}

#[derive(Args)]
struct BuildWasm {
    /// Compile the library more quickly, with fewer optimizations
    /// and more runtime assertions.
    #[arg(long, short = '0')]
    debug: bool,
    /// Run emscripten using docker, even if \`emcc\` is installed.
    /// By default, \`emcc\` will be run directly when available.
    #[arg(long, short)]
    docker: bool,
    /// Run emscripten with verbose output.
    #[arg(long, short)]
    verbose: bool,
    /// Rebuild when relevant files are changed.
    #[arg(long, short)]
    watch: bool,
    /// Emit TypeScript type definitions for the generated bindings,
    /// requires `tsc` to be available.
    #[arg(long, short)]
    emit_tsd: bool,
    /// Generate `CommonJS` modules instead of ES modules.
    #[arg(long, short, env = "CJS")]
    cjs: bool,
}

#[derive(Args)]
struct BumpVersion {
    /// The version to bump to.
    #[arg(long, short)]
    version: Option<Version>,
}

#[derive(Args)]
struct CheckWasmExports {
    /// Recheck when relevant files are changed.
    #[arg(long, short)]
    watch: bool,
}

#[derive(Args)]
struct Clippy {
    /// Automatically apply lint suggestions (`clippy --fix`).
    #[arg(long, short)]
    fix: bool,
    /// The package to run Clippy against (`cargo -p <PACKAGE> clippy`).
    #[arg(long, short)]
    package: Option<String>,
}

#[derive(Args)]
struct GenerateFixtures {
    /// Generates the parser to WASM
    #[arg(long, short)]
    wasm: bool,
    /// Run emscripten via docker even if it is installed locally.
    #[arg(long, short, requires = "wasm")]
    docker: bool,
}

#[derive(Args)]
struct Test {
    /// Compile C code with the Clang address sanitizer.
    #[arg(long, short)]
    address_sanitizer: bool,
    /// Run only the corpus tests for the given language.
    #[arg(long, short)]
    language: Option<String>,
    /// Run only the corpus tests whose name contain the given string.
    #[arg(long, short)]
    example: Option<String>,
    /// Run the given number of iterations of randomized tests (default 10).
    #[arg(long, short)]
    iterations: Option<u32>,
    /// Set the seed used to control random behavior.
    #[arg(long, short)]
    seed: Option<usize>,
    /// Print parsing log to stderr.
    #[arg(long, short)]
    debug: bool,
    /// Generate an SVG graph of parsing logs.
    #[arg(long, short = 'D')]
    debug_graph: bool,
    /// Run the tests with a debugger.
    #[arg(short)]
    g: bool,
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
    /// Don't capture the output
    #[arg(long)]
    nocapture: bool,
    /// Enable the wasm tests.
    #[arg(long, short)]
    wasm: bool,
}

#[derive(Args)]
struct UpgradeWasmtime {
    /// The version to upgrade to.
    #[arg(long, short)]
    version: Version,
}

const BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_SHA: Option<&str> = option_env!("BUILD_SHA");
const EMSCRIPTEN_VERSION: &str = include_str!("../../cli/loader/emscripten-version");
const EMSCRIPTEN_TAG: &str = concat!(
    "docker.io/emscripten/emsdk:",
    include_str!("../../cli/loader/emscripten-version")
);

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

    let cli = Command::new("xtask")
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
    let command = Commands::from_arg_matches(&Commands::augment_subcommands(cli).get_matches())?;

    match command {
        Commands::Benchmark(benchmark_options) => benchmark::run(&benchmark_options)?,
        Commands::BuildWasm(build_wasm_options) => build_wasm::run_wasm(&build_wasm_options)?,
        Commands::BuildWasmStdlib => build_wasm::run_wasm_stdlib()?,
        Commands::BumpVersion(bump_options) => bump::run(bump_options)?,
        Commands::CheckWasmExports(check_options) => check_wasm_exports::run(&check_options)?,
        Commands::Clippy(clippy_options) => clippy::run(&clippy_options)?,
        Commands::FetchEmscripten => fetch::run_emscripten()?,
        Commands::FetchFixtures => fetch::run_fixtures()?,
        Commands::GenerateBindings => generate::run_bindings()?,
        Commands::GenerateFixtures(generate_fixtures_options) => {
            generate::run_fixtures(&generate_fixtures_options)?;
        }
        Commands::GenerateWasmExports => generate::run_wasm_exports()?,
        Commands::Test(test_options) => test::run(&test_options)?,
        Commands::TestWasm => test::run_wasm()?,
        Commands::UpgradeWasmtime(upgrade_wasmtime_options) => {
            upgrade_wasmtime::run(&upgrade_wasmtime_options)?;
        }
        Commands::UpgradeEmscripten => upgrade_emscripten::run()?,
    }

    Ok(())
}

fn bail_on_err(output: &std::process::Output, prefix: &str) -> Result<()> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("{prefix}:\n{stderr}");
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

pub fn create_commit(repo: &Repository, msg: &str, paths: &[&str]) -> Result<Oid> {
    let mut index = repo.index()?;
    for path in paths {
        index.add_path(Path::new(path))?;
    }

    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let signature = repo.signature()?;
    let parent_commit = repo.revparse_single("HEAD")?.peel_to_commit()?;

    Ok(repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        msg,
        &tree,
        &[&parent_commit],
    )?)
}

#[macro_export]
macro_rules! watch_wasm {
    ($watch_fn:expr) => {
        if let Err(e) = $watch_fn() {
            eprintln!("{e}");
        } else {
            println!("Build succeeded");
        }

        let watch_files = [
            "lib/tree-sitter.c",
            "lib/exports.txt",
            "lib/imports.js",
            "lib/prefix.js",
        ]
        .iter()
        .map(PathBuf::from)
        .collect::<HashSet<PathBuf>>();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut debouncer = new_debouncer(Duration::from_secs(1), None, tx)?;
        debouncer.watch("lib/binding_web", RecursiveMode::NonRecursive)?;

        for result in rx {
            match result {
                Ok(events) => {
                    for event in events {
                        if event.kind == EventKind::Access(AccessKind::Close(AccessMode::Write))
                            && event
                                .paths
                                .iter()
                                .filter_map(|p| p.file_name())
                                .any(|p| watch_files.contains(&PathBuf::from(p)))
                        {
                            if let Err(e) = $watch_fn() {
                                eprintln!("{e}");
                            } else {
                                println!("Build succeeded");
                            }
                        }
                    }
                }
                Err(errors) => {
                    return Err(anyhow!(
                        "{}",
                        errors
                            .into_iter()
                            .map(|e| e.to_string())
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
            }
        }
    };
}
