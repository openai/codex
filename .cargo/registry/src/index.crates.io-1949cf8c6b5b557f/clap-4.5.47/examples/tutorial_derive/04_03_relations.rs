use clap::{Args, Parser};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(flatten)]
    vers: Vers,

    /// some regular input
    #[arg(group = "input")]
    input_file: Option<String>,

    /// some special input argument
    #[arg(long, group = "input")]
    spec_in: Option<String>,

    #[arg(short, requires = "input")]
    config: Option<String>,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct Vers {
    /// set version manually
    #[arg(long, value_name = "VER")]
    set_ver: Option<String>,

    /// auto inc major
    #[arg(long)]
    major: bool,

    /// auto inc minor
    #[arg(long)]
    minor: bool,

    /// auto inc patch
    #[arg(long)]
    patch: bool,
}

fn main() {
    let cli = Cli::parse();

    // Let's assume the old version 1.2.3
    let mut major = 1;
    let mut minor = 2;
    let mut patch = 3;

    // See if --set_ver was used to set the version manually
    let vers = &cli.vers;
    let version = if let Some(ver) = vers.set_ver.as_deref() {
        ver.to_string()
    } else {
        // Increment the one requested (in a real program, we'd reset the lower numbers)
        let (maj, min, pat) = (vers.major, vers.minor, vers.patch);
        match (maj, min, pat) {
            (true, _, _) => major += 1,
            (_, true, _) => minor += 1,
            (_, _, true) => patch += 1,
            _ => unreachable!(),
        };
        format!("{major}.{minor}.{patch}")
    };

    println!("Version: {version}");

    // Check for usage of -c
    if let Some(config) = cli.config.as_deref() {
        let input = cli
            .input_file
            .as_deref()
            .unwrap_or_else(|| cli.spec_in.as_deref().unwrap());
        println!("Doing work using input {input} and config {config}");
    }
}
