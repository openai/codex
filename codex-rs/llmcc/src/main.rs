use clap::Parser;

use llmcc_rust::LangRust;
use llmcc_python::LangPython;
use llmcc::{run_main, LlmccOptions};

#[derive(Parser, Debug)]
#[command(name = "llmcc")]
#[command(about = "llmcc: llm context compiler")]
#[command(version)]
struct Args {
    /// Files to compile
    #[arg(value_name = "FILE", required_unless_present = "dir")]
    files: Vec<String>,

    /// Load all .rs files from a directory (recursive)
    #[arg(short, long, value_name = "DIR")]
    dir: Option<String>,

    /// Language to use: 'rust' or 'python'
    #[arg(long, value_name = "LANG", default_value = "rust")]
    lang: String,

    /// Print intermediate representation (IR)
    #[arg(long, default_value_t = false)]
    print_ir: bool,

    /// Print project graph
    #[arg(long, default_value_t = false)]
    print_graph: bool,

    /// Name of the symbol/function to query (enables find_depends mode)
    #[arg(long, value_name = "NAME")]
    query: Option<String>,

    /// Search recursively for transitive dependencies (default: direct dependencies only)
    #[arg(long, default_value_t = false)]
    recursive: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let opts = LlmccOptions {
        files: args.files,
        dir: args.dir,
        print_ir: args.print_ir,
        print_graph: args.print_graph,
        query: args.query,
        recursive: args.recursive,
    };

    let result = match args.lang.as_str() {
        "rust" => run_main::<LangRust>(&opts),
        "python" => run_main::<LangPython>(&opts),
        _ => Err(format!("Unknown language: {}", args.lang).into()),
    }?;

    if let Some(output) = result {
        println!("{}", output);
    }

    Ok(())
}
