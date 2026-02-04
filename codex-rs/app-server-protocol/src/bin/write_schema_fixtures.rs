use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "Regenerate vendored app-server schema fixtures")]
struct Args {
    /// Root directory containing `typescript/` and `json/`.
    #[arg(long = "schema-root", value_name = "DIR")]
    schema_root: Option<PathBuf>,

    /// Optional path to the Prettier executable to format generated TypeScript files.
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,

    /// Include experimental API methods and fields in generated fixtures.
    #[arg(long = "experimental")]
    experimental: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let schema_root = args
        .schema_root
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema"));

    codex_app_server_protocol::write_schema_fixtures_with_options(
        &schema_root,
        args.prettier.as_deref(),
        codex_app_server_protocol::SchemaFixtureOptions {
            experimental_api: args.experimental,
        },
    )
    .with_context(|| {
        format!(
            "failed to regenerate schema fixtures under {}",
            schema_root.display()
        )
    })
}
