//! Find the right cursor file from a cursor name

// Based on libxcb-cursor's load_cursor.c which has:
//
//   Copyright © 2013 Michael Stapelberg
//   Copyright © 2002 Keith Packard
//
// and is licensed under MIT/X Consortium License

use std::env::{var, var_os};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, Error as IOError};
use std::path::{Path, PathBuf};

static CORE_CURSORS: &[(&str, u16)] = &[
    ("X_cursor", 0),
    ("arrow", 1),
    ("based_arrow_down", 2),
    ("based_arrow_up", 3),
    ("boat", 4),
    ("bogosity", 5),
    ("bottom_left_corner", 6),
    ("bottom_right_corner", 7),
    ("bottom_side", 8),
    ("bottom_tee", 9),
    ("box_spiral", 10),
    ("center_ptr", 11),
    ("circle", 12),
    ("clock", 13),
    ("coffee_mug", 14),
    ("cross", 15),
    ("cross_reverse", 16),
    ("crosshair", 17),
    ("diamond_cross", 18),
    ("dot", 19),
    ("dotbox", 20),
    ("double_arrow", 21),
    ("draft_large", 22),
    ("draft_small", 23),
    ("draped_box", 24),
    ("exchange", 25),
    ("fleur", 26),
    ("gobbler", 27),
    ("gumby", 28),
    ("hand1", 29),
    ("hand2", 30),
    ("heart", 31),
    ("icon", 32),
    ("iron_cross", 33),
    ("left_ptr", 34),
    ("left_side", 35),
    ("left_tee", 36),
    ("leftbutton", 37),
    ("ll_angle", 38),
    ("lr_angle", 39),
    ("man", 40),
    ("middlebutton", 41),
    ("mouse", 42),
    ("pencil", 43),
    ("pirate", 44),
    ("plus", 45),
    ("question_arrow", 46),
    ("right_ptr", 47),
    ("right_side", 48),
    ("right_tee", 49),
    ("rightbutton", 50),
    ("rtl_logo", 51),
    ("sailboat", 52),
    ("sb_down_arrow", 53),
    ("sb_h_double_arrow", 54),
    ("sb_left_arrow", 55),
    ("sb_right_arrow", 56),
    ("sb_up_arrow", 57),
    ("sb_v_double_arrow", 58),
    ("shuttle", 59),
    ("sizing", 60),
    ("spider", 61),
    ("spraycan", 62),
    ("star", 63),
    ("target", 64),
    ("tcross", 65),
    ("top_left_arrow", 66),
    ("top_left_corner", 67),
    ("top_right_corner", 68),
    ("top_side", 69),
    ("top_tee", 70),
    ("trek", 71),
    ("ul_angle", 72),
    ("umbrella", 73),
    ("ur_angle", 74),
    ("watch", 75),
    ("xterm", 76),
];

/// Find a core cursor based on its name
///
/// This function checks a built-in list of known names.
fn cursor_shape_to_id(name: &str) -> Option<u16> {
    CORE_CURSORS
        .iter()
        .filter(|&(name2, _)| name == *name2)
        .map(|&(_, id)| id)
        .next()
}

/// An error that occurred while searching
#[derive(Debug)]
pub(crate) enum Error {
    /// `$HOME` is not set
    NoHomeDir,

    /// No cursor file could be found
    NothingFound,
}

/// The result of finding a cursor
#[derive(Debug)]
pub(crate) enum Cursor<F> {
    /// The cursor is a core cursor that can be created with xproto's `CreateGlyphCursor`
    CoreChar(u16),

    /// A cursor file was opened
    File(F),
}

// Get the 'Inherits' entry from an index.theme file
fn parse_inherits(filename: &Path) -> Result<Vec<String>, IOError> {
    let file = File::open(filename)?;
    parse_inherits_impl(&mut BufReader::new(file))
}

// Get the 'Inherits' entry from an index.theme file
fn parse_inherits_impl(input: &mut impl BufRead) -> Result<Vec<String>, IOError> {
    let mut buffer = Vec::new();
    loop {
        buffer.clear();

        // Read a line
        if 0 == input.read_until(b'\n', &mut buffer)? {
            // End of file, return an empty result
            return Ok(Default::default());
        }

        // Remove end of line marker
        if buffer.last() == Some(&b'\n') {
            let _ = buffer.pop();
        }

        let begin = b"Inherits";
        if buffer.starts_with(begin) {
            let mut result = Vec::new();

            let mut to_parse = &buffer[begin.len()..];

            fn skip_while(mut slice: &[u8], f: impl Fn(u8) -> bool) -> &[u8] {
                while !slice.is_empty() && f(slice[0]) {
                    slice = &slice[1..]
                }
                slice
            }

            // Skip all spaces
            to_parse = skip_while(to_parse, |c| c == b' ');

            // Now we need an equal sign
            if to_parse.first() == Some(&b'=') {
                to_parse = &to_parse[1..];

                fn should_skip(c: u8) -> bool {
                    matches!(c, b' ' | b'\t' | b'\n' | b';' | b',')
                }

                // Iterate over the pieces
                for mut part in to_parse.split(|&x| x == b':') {
                    // Skip all leading whitespace
                    part = skip_while(part, should_skip);

                    // Skip all trailing whitespace
                    while let Some((&last, rest)) = part.split_last() {
                        if !should_skip(last) {
                            break;
                        }
                        part = rest;
                    }
                    if !part.is_empty() {
                        if let Ok(part) = std::str::from_utf8(part) {
                            result.push(part.to_string());
                        }
                    }
                }
            }
            return Ok(result);
        }
    }
}

#[cfg(test)]
mod test_parse_inherits {
    use super::parse_inherits_impl;
    use std::io::Cursor;

    #[test]
    fn parse_inherits_successful() {
        let input =
            b"Hi\nInherits = \t ; whatever ;,::; stuff : i s ,: \tthis \t \nInherits=ignored\n";
        let mut input = Cursor::new(&input[..]);
        let result = parse_inherits_impl(&mut input).unwrap();
        assert_eq!(result, vec!["whatever", "stuff", "i s", "this"]);
    }
}

/// Find a cursor file based on the name of a cursor theme and the name of the cursor.
pub(crate) fn find_cursor(theme: &str, name: &str) -> Result<Cursor<File>, Error> {
    let home = match var_os("HOME") {
        Some(home) => home,
        None => return Err(Error::NoHomeDir),
    };
    let cursor_path = var("XCURSOR_PATH").unwrap_or_else(|_| {
        "~/.icons:/usr/share/icons:/usr/share/pixmaps:/usr/X11R6/lib/X11/icons".into()
    });
    let open_cursor = |file: &Path| File::open(file);
    find_cursor_impl(
        &home,
        &cursor_path,
        theme,
        name,
        open_cursor,
        parse_inherits,
    )
}

fn find_cursor_impl<F, G, H>(
    home: &OsStr,
    cursor_path: &str,
    theme: &str,
    name: &str,
    mut open_cursor: G,
    mut parse_inherits: H,
) -> Result<Cursor<F>, Error>
where
    G: FnMut(&Path) -> Result<F, IOError>,
    H: FnMut(&Path) -> Result<Vec<String>, IOError>,
{
    if theme == "core" {
        if let Some(id) = cursor_shape_to_id(name) {
            return Ok(Cursor::CoreChar(id));
        }
    }

    let cursor_path = cursor_path.split(':').collect::<Vec<_>>();

    let mut next_inherits = Vec::new();
    let mut last_inherits = vec![theme.into()];
    while !last_inherits.is_empty() {
        for theme in last_inherits {
            for path in &cursor_path {
                // Calculate the path to the theme's directory
                let mut theme_dir = PathBuf::new();
                // Does the path begin with '~'?
                if let Some(mut path) = path.strip_prefix('~') {
                    theme_dir.push(home);
                    // Skip a path separator if there is one
                    if path.chars().next().map(std::path::is_separator) == Some(true) {
                        path = &path[1..];
                    }
                    theme_dir.push(path);
                } else {
                    theme_dir.push(path);
                }
                theme_dir.push(&theme);

                // Find the cursor in the theme
                let mut cursor_file = theme_dir.clone();
                cursor_file.push("cursors");
                cursor_file.push(name);
                if let Ok(file) = open_cursor(&cursor_file) {
                    return Ok(Cursor::File(file));
                }

                // Get the theme's index.theme file and parse its 'Inherits' line
                let mut index = theme_dir;
                index.push("index.theme");
                if let Ok(res) = parse_inherits(&index) {
                    next_inherits.extend(res);
                }
            }
        }

        last_inherits = next_inherits;
        next_inherits = Vec::new();
    }

    Err(Error::NothingFound)
}

// FIXME: Make these tests pass on Windows; problem is "/" vs "\\" in paths
#[cfg(all(test, unix))]
mod test_find_cursor {
    use super::{find_cursor_impl, Cursor, Error};
    use crate::errors::ConnectionError;
    use std::io::{Error as IOError, ErrorKind};
    use std::path::Path;

    #[test]
    fn core_cursor() {
        let cb1 = |_: &Path| -> Result<(), _> { unimplemented!() };
        let cb2 = |_: &Path| unimplemented!();
        match find_cursor_impl("unused".as_ref(), "unused", "core", "heart", cb1, cb2).unwrap() {
            Cursor::CoreChar(31) => {}
            e => panic!("Unexpected result {:?}", e),
        }
    }

    #[test]
    fn nothing_found() {
        let mut opened = Vec::new();
        let mut inherit_parsed = Vec::new();
        let cb1 = |path: &Path| -> Result<(), _> {
            opened.push(path.to_str().unwrap().to_owned());
            Err(IOError::new(
                ErrorKind::Other,
                ConnectionError::UnknownError,
            ))
        };
        let cb2 = |path: &Path| {
            inherit_parsed.push(path.to_str().unwrap().to_owned());
            Ok(Vec::new())
        };
        match find_cursor_impl(
            "home".as_ref(),
            "path:~/some/:/entries",
            "theme",
            "theCursor",
            cb1,
            cb2,
        ) {
            Err(Error::NothingFound) => {}
            e => panic!("Unexpected result {:?}", e),
        }
        assert_eq!(
            opened,
            &[
                "path/theme/cursors/theCursor",
                "home/some/theme/cursors/theCursor",
                "/entries/theme/cursors/theCursor",
            ]
        );
        assert_eq!(
            inherit_parsed,
            &[
                "path/theme/index.theme",
                "home/some/theme/index.theme",
                "/entries/theme/index.theme",
            ]
        );
    }

    #[test]
    fn inherit() {
        let mut opened = Vec::new();
        let cb1 = |path: &Path| -> Result<(), _> {
            opened.push(path.to_str().unwrap().to_owned());
            Err(IOError::new(
                ErrorKind::Other,
                ConnectionError::UnknownError,
            ))
        };
        let cb2 = |path: &Path| {
            if path.starts_with("base/theTheme") {
                Ok(vec!["inherited".into()])
            } else if path.starts_with("path/inherited") {
                Ok(vec!["theEnd".into()])
            } else {
                Ok(vec![])
            }
        };
        match find_cursor_impl(
            "home".as_ref(),
            "path:base:tail",
            "theTheme",
            "theCursor",
            cb1,
            cb2,
        ) {
            Err(Error::NothingFound) => {}
            e => panic!("Unexpected result {:?}", e),
        }
        assert_eq!(
            opened,
            &[
                "path/theTheme/cursors/theCursor",
                "base/theTheme/cursors/theCursor",
                "tail/theTheme/cursors/theCursor",
                "path/inherited/cursors/theCursor",
                "base/inherited/cursors/theCursor",
                "tail/inherited/cursors/theCursor",
                "path/theEnd/cursors/theCursor",
                "base/theEnd/cursors/theCursor",
                "tail/theEnd/cursors/theCursor",
            ]
        );
    }
}
