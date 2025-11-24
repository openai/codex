// Copyright 2019 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! ncurses-compatible database discovery
//!
//! Does not support hashed database, only filesystem!

use std::env;
use std::fs;
use std::path::PathBuf;

use dirs_next as dirs;

/// Return path to database entry for `term`
pub fn get_dbpath_for_term(term: &str) -> Option<PathBuf> {
    let mut dirs_to_search = Vec::new();
    let first_char = match term.chars().next() {
        Some(c) => c,
        None => return None,
    };

    // Find search directory
    // The terminfo manual says:
    //
    // > If  the  environment  variable  TERMINFO is set, it is interpreted
    // > as the pathname of a directory containing the compiled description
    // > you are working on.  Only that directory is searched.
    //
    // However, the ncurses manual says:
    //
    // > If the environment variable TERMINFO is defined, any program using
    // > curses checks for a local terminal definition  before  checking in
    // > the standard place.
    //
    // Given that ncurses is the defacto standard, we follow the ncurses manual.
    if let Some(dir) = env::var_os("TERMINFO") {
        dirs_to_search.push(PathBuf::from(dir));
    }

    if let Ok(dirs) = env::var("TERMINFO_DIRS") {
        for i in dirs.split(':') {
            if i == "" {
                dirs_to_search.push(PathBuf::from("/usr/share/terminfo"));
            } else {
                dirs_to_search.push(PathBuf::from(i));
            }
        }
    } else {
        // Found nothing in TERMINFO_DIRS, use the default paths:
        // According to  /etc/terminfo/README, after looking at
        // ~/.terminfo, ncurses will search /etc/terminfo, then
        // /lib/terminfo, and eventually /usr/share/terminfo.
        // On Haiku the database can be found at /boot/system/data/terminfo
        if let Some(mut homedir) = dirs::home_dir() {
            homedir.push(".terminfo");
            dirs_to_search.push(homedir)
        }

        dirs_to_search.push(PathBuf::from("/etc/terminfo"));
        dirs_to_search.push(PathBuf::from("/lib/terminfo"));
        dirs_to_search.push(PathBuf::from("/usr/share/terminfo"));
        dirs_to_search.push(PathBuf::from("/boot/system/data/terminfo"));
    }

    // Look for the terminal in all of the search directories
    for mut p in dirs_to_search {
        if fs::metadata(&p).is_ok() {
            p.push(&first_char.to_string());
            p.push(&term);
            if fs::metadata(&p).is_ok() {
                return Some(p);
            }
            p.pop();
            p.pop();

            // on some installations the dir is named after the hex of the char
            // (e.g. OS X)
            p.push(&format!("{:x}", first_char as usize));
            p.push(term);
            if fs::metadata(&p).is_ok() {
                return Some(p);
            }
        }
    }
    None
}
