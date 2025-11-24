//! Tools for finding and manipulating differences between files
//!
//! ## Overview
//!
//! This library is intended to be a collection of tools used to find and
//! manipulate differences between files inspired by [LibXDiff] and [GNU
//! Diffutils]. Version control systems like [Git] and [Mercurial] generally
//! communicate differences between two versions of a file using a `diff` or
//! `patch`.
//!
//! The current diff implementation is based on the [Myers' diff algorithm].
//!
//! The documentation generally refers to "files" in many places but none of
//! the apis explicitly operate on on-disk files. Instead this library
//! requires that the text being operated on resides in-memory and as such if
//! you want to perform operations on files, it is up to the user to load the
//! contents of those files into memory before passing their contents to the
//! apis provided by this library.
//!
//! ## UTF-8 and Non-UTF-8
//!
//! This library has support for working with both utf8 and non-utf8 texts.
//! Most of the API's have two different variants, one for working with utf8
//! `str` texts (e.g. [`create_patch`]) and one for working with bytes `[u8]`
//! which may or may not be utf8 (e.g. [`create_patch_bytes`]).
//!
//! ## Creating a Patch
//!
//! A [`Patch`] between two texts can be created by doing the following:
//!
//! ```
//! use diffy::create_patch;
//!
//! let original = "The Way of Kings\nWords of Radiance\n";
//! let modified = "The Way of Kings\nWords of Radiance\nOathbringer\n";
//!
//! let patch = create_patch(original, modified);
//! #
//! # let expected = "\
//! # --- original
//! # +++ modified
//! # @@ -1,2 +1,3 @@
//! #  The Way of Kings
//! #  Words of Radiance
//! # +Oathbringer
//! # ";
//! #
//! # assert_eq!(patch.to_string(), expected);
//! ```
//!
//! A [`Patch`] can the be output in the [Unified Format] either by using its
//! [`Display`] impl or by using a [`PatchFormatter`] to output the diff with
//! color.
//!
//! ```
//! # use diffy::create_patch;
//! #
//! # let original = "The Way of Kings\nWords of Radiance\n";
//! # let modified = "The Way of Kings\nWords of Radiance\nOathbringer\n";
//! #
//! # let patch = create_patch(original, modified);
//! #
//! # let expected = "\
//! # --- original
//! # +++ modified
//! # @@ -1,2 +1,3 @@
//! #  The Way of Kings
//! #  Words of Radiance
//! # +Oathbringer
//! # ";
//! #
//! # assert_eq!(patch.to_string(), expected);
//! #
//! // Without color
//! print!("{}", patch);
//!
//! // With color
//! # use diffy::PatchFormatter;
//! let f = PatchFormatter::new().with_color();
//! print!("{}", f.fmt_patch(&patch));
//! ```
//!
//! ```console
//! --- original
//! +++ modified
//! @@ -1,2 +1,3 @@
//!  The Way of Kings
//!  Words of Radiance
//! +Oathbringer
//! ```
//!
//! ## Applying a Patch
//!
//! Once you have a [`Patch`] you can apply it to a base image in order to
//! recover the new text. Each hunk will be applied to the base image in
//! sequence. Similarly to GNU `patch`, this implementation can detect when
//! line numbers specified in the patch are incorrect and will attempt to find
//! the correct place to apply each hunk by iterating forward and backward
//! from the given position until all context lines from a hunk match the base
//! image.
//!
//! ```
//! use diffy::{apply, Patch};
//!
//! let s = "\
//! --- a/skybreaker-ideals
//! +++ b/skybreaker-ideals
//! @@ -10,6 +10,8 @@
//!  First:
//!      Life before death,
//!      strength before weakness,
//!      journey before destination.
//!  Second:
//! -    I will put the law before all else.
//! +    I swear to seek justice,
//! +    to let it guide me,
//! +    until I find a more perfect Ideal.
//! ";
//!
//! let patch = Patch::from_str(s).unwrap();
//!
//! let base_image = "\
//! First:
//!     Life before death,
//!     strength before weakness,
//!     journey before destination.
//! Second:
//!     I will put the law before all else.
//! ";
//!
//! let expected = "\
//! First:
//!     Life before death,
//!     strength before weakness,
//!     journey before destination.
//! Second:
//!     I swear to seek justice,
//!     to let it guide me,
//!     until I find a more perfect Ideal.
//! ";
//!
//! assert_eq!(apply(base_image, &patch).unwrap(), expected);
//! ```
//!
//! ## Performing a Three-way Merge
//!
//! Two files `A` and `B` can be merged together given a common ancestor or
//! original file `O` to produce a file `C` similarly to how [diff3]
//! performs a three-way merge.
//!
//! ```console
//!     --- A ---
//!   /           \
//!  /             \
//! O               C
//!  \             /
//!   \           /
//!     --- B ---
//! ```
//!
//! If files `A` and `B` modified different regions of the original file `O`
//! (or the same region in the same way) then the files can be merged without
//! conflict.
//!
//! ```
//! use diffy::merge;
//!
//! let original = "the final empire\nThe Well of Ascension\nThe hero of ages\n";
//! let a = "The Final Empire\nThe Well of Ascension\nThe Hero of Ages\n";
//! let b = "The Final Empire\nThe Well of Ascension\nThe hero of ages\n";
//! let expected = "\
//! The Final Empire
//! The Well of Ascension
//! The Hero of Ages
//! ";
//!
//! assert_eq!(merge(original, a, b).unwrap(), expected);
//! ```
//!
//! If both files `A` and `B` modified the same region of the original file
//! `O` (and those modifications are different), it would result in a conflict
//! as it is not clear which modifications should be used in the merged
//! result.
//!
//! ```
//! use diffy::merge;
//!
//! let original = "The Final Empire\nThe Well of Ascension\nThe hero of ages\n";
//! let a = "The Final Empire\nThe Well of Ascension\nThe Hero of Ages\nSecret History\n";
//! let b = "The Final Empire\nThe Well of Ascension\nThe hero of ages\nThe Alloy of Law\n";
//! let expected = "\
//! The Final Empire
//! The Well of Ascension
//! <<<<<<< ours
//! The Hero of Ages
//! Secret History
//! ||||||| original
//! The hero of ages
//! =======
//! The hero of ages
//! The Alloy of Law
//! >>>>>>> theirs
//! ";
//!
//! assert_eq!(merge(original, a, b).unwrap_err(), expected);
//! ```
//!
//! [LibXDiff]: http://www.xmailserver.org/xdiff-lib.html
//! [Myers' diff algorithm]: http://www.xmailserver.org/diff2.pdf
//! [GNU Diffutils]: https://www.gnu.org/software/diffutils/
//! [Git]: https://git-scm.com/
//! [Mercurial]: https://www.mercurial-scm.org/
//! [Unified Format]: https://en.wikipedia.org/wiki/Diff#Unified_format
//! [diff3]: https://en.wikipedia.org/wiki/Diff3
//!
//! [`Display`]: https://doc.rust-lang.org/stable/std/fmt/trait.Display.html
//! [`Patch`]: struct.Patch.html
//! [`PatchFormatter`]: struct.PatchFormatter.html
//! [`create_patch`]: fn.create_patch.html
//! [`create_patch_bytes`]: fn.create_patch_bytes.html

mod apply;
mod diff;
mod merge;
mod patch;
mod range;
mod utils;

pub use apply::{apply, apply_bytes, ApplyError};
pub use diff::{create_patch, create_patch_bytes, DiffOptions};
pub use merge::{merge, merge_bytes, ConflictStyle, MergeOptions};
pub use patch::{Hunk, HunkRange, Line, ParsePatchError, Patch, PatchFormatter};
