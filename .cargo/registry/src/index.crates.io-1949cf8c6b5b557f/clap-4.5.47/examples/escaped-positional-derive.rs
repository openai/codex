use clap::Parser;

#[derive(Parser)] // requires `derive` feature
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short = 'f')]
    eff: bool,

    #[arg(short = 'p', value_name = "PEAR")]
    pea: Option<String>,

    #[arg(last = true)]
    slop: Vec<String>,
}

fn main() {
    let args = Cli::parse();

    // This is what will happen with `myprog -f -p=bob -- sloppy slop slop`...
    println!("-f used: {:?}", args.eff); // -f used: true
    println!("-p's value: {:?}", args.pea); // -p's value: Some("bob")
    println!("'slops' values: {:?}", args.slop); // 'slops' values: Some(["sloppy", "slop", "slop"])

    // Continued program logic goes here...
}
