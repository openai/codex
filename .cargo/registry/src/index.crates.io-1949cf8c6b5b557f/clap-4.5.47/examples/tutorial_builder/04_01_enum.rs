use clap::{arg, builder::PossibleValue, command, value_parser, ValueEnum};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Mode {
    Fast,
    Slow,
}

// Can also be derived with feature flag `derive`
impl ValueEnum for Mode {
    fn value_variants<'a>() -> &'a [Self] {
        &[Mode::Fast, Mode::Slow]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Mode::Fast => PossibleValue::new("fast").help("Run swiftly"),
            Mode::Slow => PossibleValue::new("slow").help("Crawl slowly but steadily"),
        })
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

impl std::str::FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("invalid variant: {s}"))
    }
}

fn main() {
    let matches = command!() // requires `cargo` feature
        .arg(
            arg!(<MODE>)
                .help("What mode to run the program in")
                .value_parser(value_parser!(Mode)),
        )
        .get_matches();

    // Note, it's safe to call unwrap() because the arg is required
    match matches
        .get_one::<Mode>("MODE")
        .expect("'MODE' is required and parsing will fail if its missing")
    {
        Mode::Fast => {
            println!("Hare");
        }
        Mode::Slow => {
            println!("Tortoise");
        }
    }
}
