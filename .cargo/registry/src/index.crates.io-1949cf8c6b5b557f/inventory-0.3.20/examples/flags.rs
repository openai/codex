struct Flag {
    short: char,
    name: &'static str,
}

impl Flag {
    const fn new(short: char, name: &'static str) -> Self {
        Flag { short, name }
    }
}

inventory::submit! {
    Flag::new('v', "verbose")
}

inventory::collect!(Flag);

fn main() {
    for flag in inventory::iter::<Flag> {
        println!("-{}, --{}", flag.short, flag.name);
    }
}
