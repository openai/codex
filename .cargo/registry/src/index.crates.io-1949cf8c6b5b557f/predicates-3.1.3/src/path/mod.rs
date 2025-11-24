// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Path Predicates
//!
//! This module contains predicates specific to the file system.

mod existence;
pub use self::existence::{exists, missing, ExistencePredicate};
mod ft;
pub use self::ft::{is_dir, is_file, is_symlink, FileTypePredicate};
mod fc;
pub use self::fc::{FileContentPredicate, PredicateFileContentExt};
mod fs;
pub use self::fs::{eq_file, BinaryFilePredicate, StrFilePredicate};
