use fallible_iterator::FallibleIterator;
use std::borrow::Cow;
use std::io::{BufRead, Lines, StdinLock, Write};
use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, Command};

use addr2line::{Loader, LoaderReader, Location};

fn parse_uint_from_hex_string(string: &str) -> Option<u64> {
    if string.len() > 2 && string.starts_with("0x") {
        u64::from_str_radix(&string[2..], 16).ok()
    } else {
        u64::from_str_radix(string, 16).ok()
    }
}

enum Addrs<'a> {
    Args(clap::parser::ValuesRef<'a, String>),
    Stdin(Lines<StdinLock<'a>>),
    All {
        iter: addr2line::LocationRangeIter<'a, LoaderReader<'a>>,
        max: u64,
    },
}

impl<'a> Iterator for Addrs<'a> {
    type Item = Option<u64>;

    fn next(&mut self) -> Option<Option<u64>> {
        let text = match self {
            Addrs::Args(vals) => vals.next().map(Cow::from),
            Addrs::Stdin(lines) => lines.next().map(Result::unwrap).map(Cow::from),
            Addrs::All { iter, max } => {
                for (addr, _len, _loc) in iter {
                    if addr >= *max {
                        *max = addr + 1;
                        return Some(Some(addr));
                    }
                }
                return None;
            }
        };
        text.as_ref()
            .map(Cow::as_ref)
            .map(parse_uint_from_hex_string)
    }
}

fn print_loc(loc: Option<&Location<'_>>, basenames: bool, llvm: bool) {
    if let Some(loc) = loc {
        if let Some(ref file) = loc.file.as_ref() {
            let path = if basenames {
                Path::new(Path::new(file).file_name().unwrap())
            } else {
                Path::new(file)
            };
            print!("{}:", path.display());
        } else {
            print!("??:");
        }
        if llvm {
            print!("{}:{}", loc.line.unwrap_or(0), loc.column.unwrap_or(0));
        } else if let Some(line) = loc.line {
            print!("{}", line);
        } else {
            print!("?");
        }
        println!();
    } else if llvm {
        println!("??:0:0");
    } else {
        println!("??:0");
    }
}

fn print_function(name: Option<&str>, language: Option<gimli::DwLang>, demangle: bool) {
    if let Some(name) = name {
        if demangle {
            print!("{}", addr2line::demangle_auto(Cow::from(name), language));
        } else {
            print!("{}", name);
        }
    } else {
        print!("??");
    }
}

struct Options<'a> {
    do_functions: bool,
    do_inlines: bool,
    pretty: bool,
    print_addrs: bool,
    basenames: bool,
    demangle: bool,
    llvm: bool,
    exe: &'a PathBuf,
    sup: Option<&'a PathBuf>,
}

fn main() {
    let matches = Command::new("addr2line")
        .version(env!("CARGO_PKG_VERSION"))
        .about("A fast addr2line Rust port")
        .max_term_width(100)
        .args(&[
            Arg::new("exe")
                .short('e')
                .long("exe")
                .value_name("filename")
                .value_parser(clap::value_parser!(PathBuf))
                .help(
                    "Specify the name of the executable for which addresses should be translated.",
                )
                .required(true),
            Arg::new("sup")
                .long("sup")
                .value_name("filename")
                .value_parser(clap::value_parser!(PathBuf))
                .help("Path to supplementary object file."),
            Arg::new("all")
                .long("all")
                .action(ArgAction::SetTrue)
                .conflicts_with("addrs")
                .help("Display all addresses that have line number information."),
            Arg::new("functions")
                .short('f')
                .long("functions")
                .action(ArgAction::SetTrue)
                .help("Display function names as well as file and line number information."),
            Arg::new("pretty").short('p').long("pretty-print")
                .action(ArgAction::SetTrue)
                .help(
                "Make the output more human friendly: each location are printed on one line.",
            ),
            Arg::new("inlines").short('i').long("inlines")
                .action(ArgAction::SetTrue)
                .help(
                "If the address belongs to a function that was inlined, the source information for \
                all enclosing scopes back to the first non-inlined function will also be printed.",
            ),
            Arg::new("addresses").short('a').long("addresses")
                .action(ArgAction::SetTrue)
                .help(
                "Display the address before the function name, file and line number information.",
            ),
            Arg::new("basenames")
                .short('s')
                .long("basenames")
                .action(ArgAction::SetTrue)
                .help("Display only the base of each file name."),
            Arg::new("demangle").short('C').long("demangle")
                .action(ArgAction::SetTrue)
                .help(
                "Demangle function names. \
                Specifying a specific demangling style (like GNU addr2line) is not supported. \
                (TODO)"
            ),
            Arg::new("llvm")
                .long("llvm")
                .action(ArgAction::SetTrue)
                .help("Display output in the same format as llvm-symbolizer."),
            Arg::new("addrs")
                .action(ArgAction::Append)
                .help("Addresses to use instead of reading from stdin."),
        ])
        .get_matches();

    let opts = Options {
        do_functions: matches.get_flag("functions"),
        do_inlines: matches.get_flag("inlines"),
        pretty: matches.get_flag("pretty"),
        print_addrs: matches.get_flag("addresses"),
        basenames: matches.get_flag("basenames"),
        demangle: matches.get_flag("demangle"),
        llvm: matches.get_flag("llvm"),
        exe: matches.get_one::<PathBuf>("exe").unwrap(),
        sup: matches.get_one::<PathBuf>("sup"),
    };

    let ctx = Loader::new_with_sup(opts.exe, opts.sup).unwrap();

    let stdin = std::io::stdin();
    let addrs = if matches.get_flag("all") {
        Addrs::All {
            iter: ctx.find_location_range(0, !0).unwrap(),
            max: 0,
        }
    } else {
        matches
            .get_many::<String>("addrs")
            .map(Addrs::Args)
            .unwrap_or_else(|| Addrs::Stdin(stdin.lock().lines()))
    };

    for probe in addrs {
        if opts.print_addrs {
            let addr = probe.unwrap_or(0);
            if opts.llvm {
                print!("0x{:x}", addr);
            } else {
                print!("0x{:016x}", addr);
            }
            if opts.pretty {
                print!(": ");
            } else {
                println!();
            }
        }

        if opts.do_functions || opts.do_inlines {
            let mut printed_anything = false;
            if let Some(probe) = probe {
                let mut frames = ctx.find_frames(probe).unwrap().peekable();
                let mut first = true;
                while let Some(frame) = frames.next().unwrap() {
                    if opts.pretty && !first {
                        print!(" (inlined by) ");
                    }
                    first = false;

                    if opts.do_functions {
                        // Only use the symbol table if this isn't an inlined function.
                        let symbol = if matches!(frames.peek(), Ok(None)) {
                            ctx.find_symbol(probe)
                        } else {
                            None
                        };
                        if symbol.is_some() {
                            // Prefer the symbol table over the DWARF name because:
                            // - the symbol can include a clone suffix
                            // - llvm may omit the linkage name in the DWARF with -g1
                            print_function(symbol, None, opts.demangle);
                        } else if let Some(func) = frame.function {
                            print_function(
                                func.raw_name().ok().as_deref(),
                                func.language,
                                opts.demangle,
                            );
                        } else {
                            print_function(None, None, opts.demangle);
                        }

                        if opts.pretty {
                            print!(" at ");
                        } else {
                            println!();
                        }
                    }

                    print_loc(frame.location.as_ref(), opts.basenames, opts.llvm);

                    printed_anything = true;

                    if !opts.do_inlines {
                        break;
                    }
                }
            }

            if !printed_anything {
                if opts.do_functions {
                    let name = probe.and_then(|probe| ctx.find_symbol(probe));
                    print_function(name, None, opts.demangle);

                    if opts.pretty {
                        print!(" at ");
                    } else {
                        println!();
                    }
                }

                print_loc(None, opts.basenames, opts.llvm);
            }
        } else {
            let loc = probe.and_then(|probe| ctx.find_location(probe).unwrap());
            print_loc(loc.as_ref(), opts.basenames, opts.llvm);
        }

        if opts.llvm {
            println!();
        }
        std::io::stdout().flush().unwrap();
    }
}
