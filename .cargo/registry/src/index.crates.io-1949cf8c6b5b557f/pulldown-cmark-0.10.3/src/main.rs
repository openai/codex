// Copyright 2015 Google Inc. All rights reserved.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Command line tool to exercise pulldown-cmark.

#![forbid(unsafe_code)]

use pulldown_cmark::{html, BrokenLink, Options, Parser};

use std::env;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;

fn dry_run(text: &str, opts: Options, broken_links: &mut Vec<BrokenLink<'static>>) {
    let p = Parser::new_with_broken_link_callback(
        text,
        opts,
        Some(|link: BrokenLink<'_>| {
            broken_links.push(link.into_static());
            None
        }),
    );
    let count = p.count();
    println!("{} events", count);
}

fn print_events(text: &str, opts: Options, broken_links: &mut Vec<BrokenLink<'static>>) {
    let parser = Parser::new_with_broken_link_callback(
        text,
        opts,
        Some(|link: BrokenLink<'_>| {
            broken_links.push(link.into_static());
            None
        }),
    )
    .into_offset_iter();
    for (event, range) in parser {
        println!("{:?}: {:?}", range, event);
    }
    println!("EOF");
}

fn brief(program: &str) -> String {
    format!(
        "Usage: {} [options]\n\n{}",
        program, "Reads markdown from file or standard input and emits HTML.",
    )
}

pub fn main() -> std::io::Result<()> {
    let args: Vec<_> = env::args().collect();
    let mut opts = getopts::Options::new();
    opts.optflag("h", "help", "this help message");
    opts.optflag("d", "dry-run", "dry run, produce no output");
    opts.optflag("e", "events", "print event sequence instead of rendering");
    opts.optflag("T", "enable-tables", "enable GitHub-style tables");
    opts.optflag("F", "enable-footnotes", "enable GitHub-style footnotes");
    opts.optflag("", "enable-old-footnotes", "enable Hoedown-style footnotes");
    opts.optflag(
        "S",
        "enable-strikethrough",
        "enable GitHub-style strikethrough",
    );
    opts.optflag("L", "enable-tasklists", "enable GitHub-style task lists");
    opts.optflag("P", "enable-smart-punctuation", "enable smart punctuation");
    opts.optflag(
        "H",
        "enable-heading-attributes",
        "enable heading attributes",
    );
    opts.optflag("M", "enable-metadata-blocks", "enable metadata blocks");
    opts.optflag(
        "R",
        "reject-broken-links",
        "fail if input file has broken links",
    );

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}\n{}", f, opts.usage(&brief(&args[0])));
            std::process::exit(1);
        }
    };
    if matches.opt_present("help") {
        println!("{}", opts.usage(&brief(&args[0])));
        return Ok(());
    }
    let mut opts = Options::empty();
    if matches.opt_present("enable-tables") {
        opts.insert(Options::ENABLE_TABLES);
    }
    if matches.opt_present("enable-footnotes") {
        opts.insert(Options::ENABLE_FOOTNOTES);
    }
    if matches.opt_present("enable-old-footnotes") {
        opts.insert(Options::ENABLE_OLD_FOOTNOTES);
    }
    if matches.opt_present("enable-strikethrough") {
        opts.insert(Options::ENABLE_STRIKETHROUGH);
    }
    if matches.opt_present("enable-tasklists") {
        opts.insert(Options::ENABLE_TASKLISTS);
    }
    if matches.opt_present("enable-smart-punctuation") {
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    }
    if matches.opt_present("enable-heading-attributes") {
        opts.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    }
    if matches.opt_present("enable-metadata-blocks") {
        opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
        opts.insert(Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS);
    }

    let mut input = String::new();
    let mut broken_links = vec![];
    if !&matches.free.is_empty() {
        for filename in &matches.free {
            let real_path = PathBuf::from(filename);
            let mut f = File::open(&real_path).expect("file not found");
            f.read_to_string(&mut input)
                .expect("something went wrong reading the file");
            if matches.opt_present("events") {
                print_events(&input, opts, &mut broken_links);
            } else if matches.opt_present("dry-run") {
                dry_run(&input, opts, &mut broken_links);
            } else {
                pulldown_cmark(&input, opts, &mut broken_links);
            }
        }
    } else {
        let _ = io::stdin().lock().read_to_string(&mut input);
        if matches.opt_present("events") {
            print_events(&input, opts, &mut broken_links);
        } else if matches.opt_present("dry-run") {
            dry_run(&input, opts, &mut broken_links);
        } else {
            pulldown_cmark(&input, opts, &mut broken_links);
        }
    }

    if matches.opt_present("reject-broken-links") && !broken_links.is_empty() {
        eprintln!("Error: {} broken links:", broken_links.len());
        for link in broken_links {
            let start = link.span.start;
            let end = link.span.end;
            let reference = link.reference;
            eprintln!("[{start}-{end}]: {reference}");
        }
        std::process::exit(1);
    }

    Ok(())
}

pub fn pulldown_cmark(input: &str, opts: Options, broken_links: &mut Vec<BrokenLink<'static>>) {
    let mut p = Parser::new_with_broken_link_callback(
        input,
        opts,
        Some(|link: BrokenLink<'_>| {
            broken_links.push(link.into_static());
            None
        }),
    );
    let stdio = io::stdout();
    let buffer = std::io::BufWriter::with_capacity(1024 * 1024, stdio.lock());
    let _ = html::write_html(buffer, &mut p);
}
