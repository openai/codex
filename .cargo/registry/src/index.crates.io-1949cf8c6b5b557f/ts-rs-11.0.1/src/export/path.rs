use std::path::{Component as C, Path, PathBuf};

use super::ExportError as E;

const ERROR_MESSAGE: &str = r#"The path provided with `#[ts(export_to = "..")]` is not valid"#;

pub fn absolute<T: AsRef<Path>>(path: T) -> Result<PathBuf, E> {
    let path = std::env::current_dir()?.join(path.as_ref());

    let mut out = Vec::new();
    for comp in path.components() {
        match comp {
            C::CurDir => (),
            C::ParentDir => {
                out.pop().ok_or(E::CannotBeExported(ERROR_MESSAGE))?;
            }
            comp => out.push(comp),
        }
    }

    Ok(if !out.is_empty() {
        out.iter().collect()
    } else {
        PathBuf::from(".")
    })
}

// Construct a relative path from a provided base directory path to the provided path.
//
// Copyright 2012-2015 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Adapted from rustc's path_relative_from
// https://github.com/rust-lang/rust/blob/e1d0de82cc40b666b88d4a6d2c9dcbc81d7ed27f/src/librustc_back/rpath.rs#L116-L158
pub(super) fn diff_paths<P, B>(path: P, base: B) -> Result<PathBuf, E>
where
    P: AsRef<Path>,
    B: AsRef<Path>,
{
    let path = absolute(path)?;
    let base = absolute(base)?;

    let mut ita = path.components();
    let mut itb = base.components();
    let mut comps: Vec<C> = vec![];

    loop {
        match (ita.next(), itb.next()) {
            (Some(C::ParentDir | C::CurDir), _) | (_, Some(C::ParentDir | C::CurDir)) => {
                unreachable!(
                    "The paths have been cleaned, no no '.' or '..' components are present"
                )
            }
            (None, None) => break,
            (Some(a), None) => {
                comps.push(a);
                comps.extend(ita.by_ref());
                break;
            }
            (None, _) => comps.push(C::ParentDir),
            (Some(a), Some(b)) if comps.is_empty() && a == b => (),
            (Some(a), Some(_)) => {
                comps.push(C::ParentDir);
                for _ in itb {
                    comps.push(C::ParentDir);
                }
                comps.push(a);
                comps.extend(ita.by_ref());
                break;
            }
        }
    }

    Ok(comps.iter().map(|c| c.as_os_str()).collect())
}
