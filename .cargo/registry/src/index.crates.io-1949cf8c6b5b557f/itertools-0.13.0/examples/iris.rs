///
/// This example parses, sorts and groups the iris dataset
/// and does some simple manipulations.
///
/// Iterators and itertools functionality are used throughout.
use itertools::Itertools;
use std::collections::HashMap;
use std::iter::repeat;
use std::num::ParseFloatError;
use std::str::FromStr;

static DATA: &str = include_str!("iris.data");

#[derive(Clone, Debug)]
struct Iris {
    name: String,
    data: [f32; 4],
}

#[allow(dead_code)] // fields are currently ignored
#[derive(Clone, Debug)]
enum ParseError {
    Numeric(ParseFloatError),
    Other(&'static str),
}

impl From<ParseFloatError> for ParseError {
    fn from(err: ParseFloatError) -> Self {
        Self::Numeric(err)
    }
}

/// Parse an Iris from a comma-separated line
impl FromStr for Iris {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iris = Self {
            name: "".into(),
            data: [0.; 4],
        };
        let mut parts = s.split(',').map(str::trim);

        // using Iterator::by_ref()
        for (index, part) in parts.by_ref().take(4).enumerate() {
            iris.data[index] = part.parse::<f32>()?;
        }
        if let Some(name) = parts.next() {
            iris.name = name.into();
        } else {
            return Err(ParseError::Other("Missing name"));
        }
        Ok(iris)
    }
}

fn main() {
    // using Itertools::fold_results to create the result of parsing
    let irises = DATA
        .lines()
        .map(str::parse)
        .fold_ok(Vec::new(), |mut v, iris: Iris| {
            v.push(iris);
            v
        });
    let mut irises = match irises {
        Err(e) => {
            println!("Error parsing: {:?}", e);
            std::process::exit(1);
        }
        Ok(data) => data,
    };

    // Sort them and group them
    irises.sort_by(|a, b| Ord::cmp(&a.name, &b.name));

    // using Iterator::cycle()
    let mut plot_symbols = "+ox".chars().cycle();
    let mut symbolmap = HashMap::new();

    // using Itertools::chunk_by
    for (species, species_chunk) in &irises.iter().chunk_by(|iris| &iris.name) {
        // assign a plot symbol
        symbolmap
            .entry(species)
            .or_insert_with(|| plot_symbols.next().unwrap());
        println!("{} (symbol={})", species, symbolmap[species]);

        for iris in species_chunk {
            // using Itertools::format for lazy formatting
            println!("{:>3.1}", iris.data.iter().format(", "));
        }
    }

    // Look at all combinations of the four columns
    //
    // See https://en.wikipedia.org/wiki/Iris_flower_data_set
    //
    let n = 30; // plot size
    let mut plot = vec![' '; n * n];

    // using Itertools::tuple_combinations
    for (a, b) in (0..4).tuple_combinations() {
        println!("Column {} vs {}:", a, b);

        // Clear plot
        //
        // using std::iter::repeat;
        // using Itertools::set_from
        plot.iter_mut().set_from(repeat(' '));

        // using Itertools::minmax
        let min_max = |data: &[Iris], col| {
            data.iter()
                .map(|iris| iris.data[col])
                .minmax()
                .into_option()
                .expect("Can't find min/max of empty iterator")
        };
        let (min_x, max_x) = min_max(&irises, a);
        let (min_y, max_y) = min_max(&irises, b);

        // Plot the data points
        let round_to_grid = |x, min, max| ((x - min) / (max - min) * ((n - 1) as f32)) as usize;
        let flip = |ix| n - 1 - ix; // reverse axis direction

        for iris in &irises {
            let ix = round_to_grid(iris.data[a], min_x, max_x);
            let iy = flip(round_to_grid(iris.data[b], min_y, max_y));
            plot[n * iy + ix] = symbolmap[&iris.name];
        }

        // render plot
        //
        // using Itertools::join
        for line in plot.chunks(n) {
            println!("{}", line.iter().join(" "))
        }
    }
}
