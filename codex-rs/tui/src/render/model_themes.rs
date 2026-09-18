//! Fixed model-named theme assets loaded through the standard TextMate parser.

use std::io::Cursor;
use syntect::highlighting::Theme;
use syntect::highlighting::ThemeSet;

pub(super) const THEMES: &[(&str, &str)] = &[
    ("ada", include_str!("../../assets/themes/ada.tmTheme")),
    (
        "babbage",
        include_str!("../../assets/themes/babbage.tmTheme"),
    ),
    ("curie", include_str!("../../assets/themes/curie.tmTheme")),
    (
        "cushman",
        include_str!("../../assets/themes/cushman.tmTheme"),
    ),
    ("dali", include_str!("../../assets/themes/dali.tmTheme")),
    (
        "davinci",
        include_str!("../../assets/themes/davinci.tmTheme"),
    ),
];

pub(super) fn resolve(name: &str) -> Option<Theme> {
    let (_, source) = THEMES.iter().find(|(key, _)| *key == name)?;
    ThemeSet::load_from_reader(&mut Cursor::new(source.as_bytes())).ok()
}
