use clap::Parser;

#[derive(Parser)] // requires `derive` feature
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
#[command(styles = CLAP_STYLING)]
enum CargoCli {
    ExampleDerive(ExampleDeriveArgs),
}

// See also `clap_cargo::style::CLAP_STYLING`
pub const CLAP_STYLING: clap::builder::styling::Styles = clap::builder::styling::Styles::styled()
    .header(clap_cargo::style::HEADER)
    .usage(clap_cargo::style::USAGE)
    .literal(clap_cargo::style::LITERAL)
    .placeholder(clap_cargo::style::PLACEHOLDER)
    .error(clap_cargo::style::ERROR)
    .valid(clap_cargo::style::VALID)
    .invalid(clap_cargo::style::INVALID);

#[derive(clap::Args)]
#[command(version, about, long_about = None)]
struct ExampleDeriveArgs {
    #[arg(long)]
    manifest_path: Option<std::path::PathBuf>,
}

fn main() {
    let CargoCli::ExampleDerive(args) = CargoCli::parse();
    println!("{:?}", args.manifest_path);
}
