// Copyright 2016 The Fancy Regex Authors.
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

//! A simple test app for exercising and debugging the regex engine.

use fancy_regex::internal::{analyze, compile, run_trace, Insn, Prog};
use fancy_regex::*;
use std::env;
use std::str::FromStr;

fn main() {
    let mut args = env::args().skip(1);
    if let Some(cmd) = args.next() {
        if cmd == "parse" {
            if let Some(re) = args.next() {
                let e = Expr::parse_tree(&re);
                println!("{:#?}", e);
            }
        } else if cmd == "analyze" {
            if let Some(re) = args.next() {
                let tree = Expr::parse_tree(&re).unwrap();
                let a = analyze(&tree);
                println!("{:#?}", a);
            }
        } else if cmd == "compile" {
            if let Some(re) = args.next() {
                let r = Regex::new(&re).unwrap();
                r.debug_print();
            }
        } else if cmd == "run" {
            let re = args.next().expect("expected regexp argument");
            let r = Regex::new(&re).unwrap();
            let text = args.next().expect("expected text argument");
            let mut pos = 0;
            if let Some(pos_str) = args.next() {
                pos = usize::from_str(&pos_str).unwrap();
            }
            if let Some(caps) = r.captures_from_pos(&text, pos).unwrap() {
                print!("captures:");
                for i in 0..caps.len() {
                    print!(" {}:", i);
                    if let Some(m) = caps.get(i) {
                        print!("[{}..{}] \"{}\"", m.start(), m.end(), m.as_str());
                    } else {
                        print!("_");
                    }
                }
                println!("");
                for cap in caps.iter() {
                    println!("iterate {:?}", cap);
                }
            } else {
                println!("no match");
            }
        } else if cmd == "trace" {
            if let Some(re) = args.next() {
                let prog = prog(&re);
                if let Some(s) = args.next() {
                    run_trace(&prog, &s, 0).unwrap();
                }
            }
        } else if cmd == "trace-inner" {
            if let Some(re) = args.next() {
                let tree = Expr::parse_tree(&re).unwrap();
                let a = analyze(&tree).unwrap();
                let p = compile(&a).unwrap();
                if let Some(s) = args.next() {
                    run_trace(&p, &s, 0).unwrap();
                }
            }
        } else if cmd == "graph" {
            let re = args.next().expect("expected regexp argument");
            graph(&re);
        } else {
            println!("commands: parse|analyze|compile|graph <expr>, run|trace|trace-inner <expr> <input>");
        }
    }
}

fn graph(re: &str) {
    let prog = prog(re);
    println!("digraph G {{");
    for (i, insn) in prog.body.iter().enumerate() {
        let label = format!("{:?}", insn)
            .replace(r#"\"#, r#"\\"#)
            .replace(r#"""#, r#"\""#);
        println!(r#"{:3} [label="{}: {}"];"#, i, i, label);
        match *insn {
            Insn::Split(a, b) => {
                println!("{:3} -> {};", i, a);
                println!("{:3} -> {};", i, b);
            }
            Insn::Jmp(target) => {
                println!("{:3} -> {};", i, target);
            }
            Insn::End => {}
            _ => {
                println!("{:3} -> {};", i, i + 1);
            }
        }
    }
    println!("}}");
}

fn prog(re: &str) -> Prog {
    let tree = Expr::parse_tree(re).expect("Expected parsing regex to work");
    let result = analyze(&tree).expect("Expected analyze to succeed");
    compile(&result).expect("Expected compile to succeed")
}
