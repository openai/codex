//! An example to parse IRI from the CLI argument.

use iri_string::types::{IriStr, RiReferenceStr, RiStr};

const USAGE: &str = "\
USAGE:
    parse [FLAGS] [--] IRI

FLAGS:
    -h, --help      Prints this help
    -i, --iri       Handle the input as an IRI (RFC 3987)
    -u, --uri       Handle the input as an URI (RFC 3986)

ARGS:
    <IRI>           IRI or URI
";

fn print_help() {
    eprintln!("{}", USAGE);
}

fn help_and_exit() -> ! {
    print_help();
    std::process::exit(1);
}

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("ERROR: {}", msg);
    eprintln!();
    print_help();
    std::process::exit(1);
}

/// Syntax specification.
#[derive(Debug, Clone, Copy)]
enum Spec {
    /// RFC 3986 URI.
    Uri,
    /// RFC 3987 IRI.
    Iri,
}

impl Default for Spec {
    #[inline]
    fn default() -> Self {
        Self::Iri
    }
}

/// CLI options.
#[derive(Default, Debug, Clone)]
struct CliOpt {
    /// IRI.
    iri: String,
    /// Syntax spec.
    spec: Spec,
}

impl CliOpt {
    fn parse() -> Self {
        let mut args = std::env::args();
        // Skip `argv[0]`.
        args.next();

        let mut iri = None;
        let mut spec = None;

        for arg in args.by_ref() {
            match arg.as_str() {
                "--iri" | "-i" => spec = Some(Spec::Iri),
                "--uri" | "-u" => spec = Some(Spec::Uri),
                "--help" | "-h" => help_and_exit(),
                opt if opt.starts_with('-') => die(format_args!("Unknown option: {}", opt)),
                _ => {
                    if iri.replace(arg).is_some() {
                        die("IRI can be specified at most once");
                    }
                }
            }
        }

        for arg in args {
            if iri.replace(arg).is_some() {
                eprintln!("ERROR: IRI can be specified at most once");
            }
        }

        let iri = iri.unwrap_or_else(|| die("IRI should be specified"));
        let spec = spec.unwrap_or_default();
        Self { iri, spec }
    }
}

fn main() {
    let opt = CliOpt::parse();

    match opt.spec {
        Spec::Iri => parse_iri(&opt),
        Spec::Uri => parse_uri(&opt),
    }
}

fn parse_iri(opt: &CliOpt) {
    let iri = parse::<iri_string::spec::IriSpec>(opt);
    let uri = iri.encode_to_uri();
    println!("ASCII:      {:?}", uri);
}

fn parse_uri(opt: &CliOpt) {
    let iri = parse::<iri_string::spec::UriSpec>(opt);
    println!("ASCII:      {:?}", iri);
}

fn parse<S: iri_string::spec::Spec>(opt: &CliOpt) -> &RiReferenceStr<S>
where
    RiStr<S>: AsRef<RiStr<iri_string::spec::IriSpec>>,
{
    let raw = &opt.iri.as_str();
    let iri = match RiReferenceStr::<S>::new(raw) {
        Ok(v) => v,
        Err(e) => die(format_args!("Failed to parse {:?}: {}", raw, e)),
    };
    println!("Successfully parsed: {:?}", iri);

    let absolute = iri.to_iri().ok();
    match absolute {
        Some(_) => println!("IRI is ablolute."),
        None => println!("IRI is relative."),
    }

    print_components(iri);
    if let Some(absolute) = absolute {
        print_normalized(absolute.as_ref());
    }

    iri
}

fn print_components<S: iri_string::spec::Spec>(iri: &RiReferenceStr<S>) {
    println!("scheme:     {:?}", iri.scheme_str());
    println!("authority:  {:?}", iri.authority_str());
    if let Some(components) = iri.authority_components() {
        println!("    userinfo: {:?}", components.userinfo());
        println!("    host:     {:?}", components.host());
        println!("    port:     {:?}", components.port());
    }
    println!("path:       {:?}", iri.path_str());
    println!("query:      {:?}", iri.query_str());
    println!("fragment:   {:?}", iri.fragment());
}

pub fn print_normalized(iri: &IriStr) {
    println!("is_normalized_rfc3986: {}", iri.is_normalized_rfc3986());
    println!(
        "is_normalized_but_authorityless_relative_path_preserved: {}",
        iri.is_normalized_but_authorityless_relative_path_preserved()
    );
    println!("normalized: {}", iri.normalize());
}
