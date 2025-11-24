extern crate phf;

use unicase::UniCase;

include!(env!("MIME_TYPES_GENERATED_PATH"));

#[cfg(feature = "rev-mappings")]
struct TopLevelExts {
    start: usize,
    end: usize,
    subs: phf::Map<UniCase<&'static str>, (usize, usize)>,
}

pub fn get_mime_types(ext: &str) -> Option<&'static [&'static str]> {
    map_lookup(&MIME_TYPES, ext).cloned()
}

pub fn get_extensions(toplevel: &str, sublevel: &str) -> Option<&'static [&'static str]> {
    if toplevel == "*" {
        return Some(EXTS);
    }

    let top = map_lookup(&REV_MAPPINGS, toplevel)?;

    if sublevel == "*" {
        return Some(&EXTS[top.start..top.end]);
    }

    let sub = map_lookup(&top.subs, sublevel)?;
    Some(&EXTS[sub.0..sub.1])
}

fn map_lookup<'key, 'map: 'key, V>(
    map: &'map phf::Map<UniCase<&'static str>, V>,
    key: &'key str,
) -> Option<&'map V> {
    // FIXME: this doesn't compile unless we transmute `key` to `UniCase<&'static str>`
    // https://github.com/sfackler/rust-phf/issues/169
    map.get(&UniCase::new(key))
}
