use log::Level;
use log::LevelFilter;

#[derive(Debug)]
pub(crate) struct Directive {
    pub(crate) name: Option<String>,
    pub(crate) level: LevelFilter,
}

// Check whether a level and target are enabled by the set of directives.
pub(crate) fn enabled(directives: &[Directive], level: Level, target: &str) -> bool {
    // Search for the longest match, the vector is assumed to be pre-sorted.
    for directive in directives.iter().rev() {
        match directive.name {
            Some(ref name) if !target.starts_with(&**name) => {}
            Some(..) | None => return level <= directive.level,
        }
    }
    false
}
