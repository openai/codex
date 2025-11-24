//! An example to normalize an IRI from the CLI argument.

use iri_string::format::ToDedicatedString;
use iri_string::types::{RiStr, RiString};

const USAGE: &str = "\
USAGE:
    normalize [FLAGS] [--] IRI

FLAGS:
    -h, --help      Prints this help
    -i, --iri       Handle the input as an IRI (RFC 3987)
    -u, --uri       Handle the input as an URI (RFC 3986)
    -a, --ascii     Converts the output to an URI (RFC 3986)
    -w, --whatwg    Serialize normalization result according to WHATWG URL Standard.

ARGS:
    <IRI>           IRI
";

fn print_help() {
    eprintln!("{USAGE}");
}

fn help_and_exit() -> ! {
    print_help();
    std::process::exit(1);
}

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("ERROR: {msg}");
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
    /// Whether to convert output to ASCII URI or not.
    output_ascii: bool,
    /// Whether to serialize in WHATWG URL Standard way.
    whatwg_serialization: bool,
}

impl CliOpt {
    fn parse() -> Self {
        let mut args = std::env::args();
        // Skip `argv[0]`.
        args.next();

        let mut iri = None;
        let mut spec = None;
        let mut output_ascii = false;
        let mut whatwg_serialization = false;

        for arg in args.by_ref() {
            match arg.as_str() {
                "--ascii" | "-a" => output_ascii = true,
                "--iri" | "-i" => spec = Some(Spec::Iri),
                "--uri" | "-u" => spec = Some(Spec::Uri),
                "--whatwg" | "-w" => whatwg_serialization = true,
                "--help" | "-h" => help_and_exit(),
                opt if opt.starts_with('-') => die(format_args!("Unknown option: {opt}")),
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
        Self {
            iri,
            spec,
            output_ascii,
            whatwg_serialization,
        }
    }
}

fn main() {
    let opt = CliOpt::parse();

    match opt.spec {
        Spec::Iri => process_iri(&opt),
        Spec::Uri => process_uri(&opt),
    }
}

fn process_iri(opt: &CliOpt) {
    let mut normalized = normalize::<iri_string::spec::IriSpec>(opt);
    if opt.output_ascii {
        normalized.encode_to_uri_inline();
    }
    println!("{normalized}");
}

fn process_uri(opt: &CliOpt) {
    let normalized = normalize::<iri_string::spec::UriSpec>(opt);
    println!("{normalized}");
}

fn normalize<S: iri_string::spec::Spec>(opt: &CliOpt) -> RiString<S> {
    let raw = &opt.iri.as_str();
    let iri = match RiStr::<S>::new(raw) {
        Ok(v) => v,
        Err(e) => die(format_args!("Failed to parse {raw:?}: {e:?}")),
    };
    let normalized = iri.normalize();
    if !opt.whatwg_serialization {
        if let Err(e) = normalized.ensure_rfc3986_normalizable() {
            die(format_args!("Failed to normalize: {e:?}"));
        }
    }
    normalized.to_dedicated_string()
}
