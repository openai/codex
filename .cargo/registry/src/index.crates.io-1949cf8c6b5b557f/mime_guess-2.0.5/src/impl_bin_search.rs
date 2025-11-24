use unicase::UniCase;

include!("mime_types.rs");
include!(env!("MIME_TYPES_GENERATED_PATH"));

#[cfg(feature = "rev-mappings")]
#[derive(Copy, Clone)]
struct TopLevelExts {
    start: usize,
    end: usize,
    subs: &'static [(UniCase<&'static str>, (usize, usize))],
}

pub fn get_mime_types(ext: &str) -> Option<&'static [&'static str]> {
    let ext = UniCase::new(ext);

    map_lookup(MIME_TYPES, &ext)
}

#[cfg(feature = "rev-mappings")]
pub fn get_extensions(toplevel: &str, sublevel: &str) -> Option<&'static [&'static str]> {
    if toplevel == "*" {
        return Some(EXTS);
    }

    let top = map_lookup(REV_MAPPINGS, toplevel)?;

    if sublevel == "*" {
        return Some(&EXTS[top.start..top.end]);
    }

    let sub = map_lookup(&top.subs, sublevel)?;
    Some(&EXTS[sub.0..sub.1])
}

fn map_lookup<K, V>(map: &'static [(K, V)], key: &str) -> Option<V>
    where K: Copy + Into<UniCase<&'static str>>, V: Copy {
    map.binary_search_by_key(&UniCase::new(key), |(k, _)| (*k).into())
        .ok()
        .map(|i| map[i].1)
}
