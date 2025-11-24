//! An example to parse IRI from the CLI argument.

use iri_string::types::{RiAbsoluteStr, RiReferenceStr};

const USAGE: &str = "\
USAGE:
    resolve [FLAGS] [--] BASE REFERENCE

FLAGS:
    -h, --help      Prints this help
    -i, --iri       Handle the input as an IRI (RFC 3987)
    -u, --uri       Handle the input as an URI (RFC 3986)
    -w, --whatwg    Serialize normalization result according to WHATWG URL Standard.

ARGS:
    <BASE>          Base IRI or URI to resolve REFERENCE against
    <REFERENCE>     IRI or URI to resolve
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
    /// Base IRI.
    base: String,
    /// Reference IRI.
    reference: String,
    /// Syntax spec.
    spec: Spec,
    /// Whether to serialize in WHATWG URL Standard way.
    whatwg_serialization: bool,
}

impl CliOpt {
    fn parse() -> Self {
        let mut args = std::env::args();
        // Skip `argv[0]`.
        args.next();

        let mut base = None;
        let mut reference = None;
        let mut spec = None;
        let mut whatwg_serialization = false;

        for arg in args.by_ref() {
            match arg.as_str() {
                "--iri" | "-i" => spec = Some(Spec::Iri),
                "--uri" | "-u" => spec = Some(Spec::Uri),
                "--whatwg" | "-w" => whatwg_serialization = true,
                "--help" | "-h" => help_and_exit(),
                opt if opt.starts_with('-') => die(format_args!("Unknown option: {}", opt)),
                _ => {
                    if base.is_none() {
                        base = Some(arg);
                    } else if reference.is_none() {
                        reference = Some(arg);
                    } else {
                        die("IRI can be specified at most twice");
                    }
                }
            }
        }

        for arg in args {
            if base.is_none() {
                base = Some(arg);
            } else if reference.is_none() {
                reference = Some(arg);
            } else {
                die("IRI can be specified at most twice");
            }
        }

        let base = base.unwrap_or_else(|| die("Base IRI should be specified"));
        let reference = reference.unwrap_or_else(|| die("Reference IRI should be specified"));
        let spec = spec.unwrap_or_default();
        Self {
            base,
            reference,
            spec,
            whatwg_serialization,
        }
    }
}

fn main() {
    let opt = CliOpt::parse();

    match opt.spec {
        Spec::Iri => parse::<iri_string::spec::IriSpec>(&opt),
        Spec::Uri => parse::<iri_string::spec::UriSpec>(&opt),
    }
}

fn parse<S: iri_string::spec::Spec>(opt: &CliOpt) {
    let base_raw = &opt.base.as_str();
    let reference_raw = &opt.reference.as_str();
    let base = match RiAbsoluteStr::<S>::new(base_raw) {
        Ok(v) => v,
        Err(e) => die(format_args!(
            "Failed to parse {:?} as an IRI (without fragment): {}",
            reference_raw, e
        )),
    };
    let reference = match RiReferenceStr::<S>::new(reference_raw) {
        Ok(v) => v,
        Err(e) => die(format_args!(
            "Failed to parse {:?} as an IRI reference: {}",
            reference_raw, e
        )),
    };

    let resolved = reference.resolve_against(base);
    if !opt.whatwg_serialization {
        if let Err(e) = resolved.ensure_rfc3986_normalizable() {
            die(format_args!(
                "Failed to resolve {:?} against {:?}: {}",
                reference_raw, base_raw, e
            ));
        }
    }
    println!("{}", resolved);
}
