pub(crate) fn windows_path_is_ambiguous(path: &str) -> bool {
    let path = path.replace('/', "\\");
    if path.starts_with(r"\??\") {
        return true;
    }
    let path = if path
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(r"\\?\UNC\"))
        || path
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(r"\\.\UNC\"))
    {
        format!(r"\\{}", &path[8..])
    } else if path.starts_with(r"\\?\") || path.starts_with(r"\\.\") {
        let path = &path[4..];
        if !matches!(path.as_bytes(), [drive, b':', b'\\', ..] if drive.is_ascii_alphabetic()) {
            return true;
        }
        path.to_string()
    } else {
        path
    };
    let bytes = path.as_bytes();
    let remainder = if matches!(bytes, [drive, b':', separator, ..]
        if drive.is_ascii_alphabetic() && *separator == b'\\')
    {
        &path[3..]
    } else {
        path.as_str()
    };
    if remainder.contains(':') {
        return true;
    }
    remainder.split('\\').any(|component| {
        !matches!(component, "." | "..")
            && (component.ends_with(['.', ' ']) || windows_reserved_component(component))
    })
}

pub(crate) fn windows_authority_path_is_ambiguous(path: &str) -> bool {
    windows_path_is_ambiguous(path)
        || path
            .replace('/', "\\")
            .split('\\')
            .any(|component| component == "..")
}

fn windows_reserved_component(component: &str) -> bool {
    let stem = component
        .trim_end_matches(['.', ' '])
        .split_once('.')
        .map_or(component, |(stem, _)| stem);
    stem.eq_ignore_ascii_case("AUX")
        || stem.eq_ignore_ascii_case("CON")
        || stem.eq_ignore_ascii_case("CONIN$")
        || stem.eq_ignore_ascii_case("CONOUT$")
        || stem.eq_ignore_ascii_case("NUL")
        || stem.eq_ignore_ascii_case("PRN")
        || ["COM", "LPT"].iter().any(|prefix| {
            let Some(rest) = stem.get(3..) else {
                return false;
            };
            let mut chars = rest.chars();
            stem.get(..3)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
                && matches!(chars.next(), Some('1'..='9' | '¹' | '²' | '³'))
                && chars.as_str().is_empty()
        })
}
