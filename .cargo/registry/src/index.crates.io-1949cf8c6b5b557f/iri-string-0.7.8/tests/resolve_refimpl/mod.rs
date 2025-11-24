//! Reference implementation based on RFC 3986 section 5.
#![cfg(feature = "alloc")]

extern crate alloc;

use alloc::format;
#[cfg(not(feature = "std"))]
use alloc::string::String;

use iri_string::spec::Spec;
use iri_string::types::{RiAbsoluteStr, RiReferenceStr, RiString};

fn to_major_components<S: Spec>(
    s: &RiReferenceStr<S>,
) -> (Option<&str>, Option<&str>, &str, Option<&str>, Option<&str>) {
    (
        s.scheme_str(),
        s.authority_str(),
        s.path_str(),
        s.query().map(|s| s.as_str()),
        s.fragment().map(|s| s.as_str()),
    )
}

/// Resolves the relative IRI.
///
/// See <https://www.rfc-editor.org/rfc/rfc3986.html#section-5.2.2>.
pub(super) fn resolve<S: Spec>(
    reference: &RiReferenceStr<S>,
    base: &RiAbsoluteStr<S>,
) -> RiString<S> {
    let (r_scheme, r_authority, r_path, r_query, r_fragment) = to_major_components(reference);
    let (b_scheme, b_authority, b_path, b_query, _) = to_major_components(base.as_ref());

    let t_scheme: &str;
    let t_authority: Option<&str>;
    let t_path: String;
    let t_query: Option<&str>;

    if let Some(r_scheme) = r_scheme {
        t_scheme = r_scheme;
        t_authority = r_authority;
        t_path = remove_dot_segments(r_path.into());
        t_query = r_query;
    } else {
        if r_authority.is_some() {
            t_authority = r_authority;
            t_path = remove_dot_segments(r_path.into());
            t_query = r_query;
        } else {
            if r_path.is_empty() {
                t_path = b_path.into();
                if r_query.is_some() {
                    t_query = r_query;
                } else {
                    t_query = b_query;
                }
            } else {
                if r_path.starts_with('/') {
                    t_path = remove_dot_segments(r_path.into());
                } else {
                    t_path = remove_dot_segments(merge(b_path, r_path, b_authority.is_some()));
                }
                t_query = r_query;
            }
            t_authority = b_authority;
        }
        t_scheme = b_scheme.expect("non-relative IRI must have a scheme");
    }
    let t_fragment: Option<&str> = r_fragment;

    let s = recompose(t_scheme, t_authority, &t_path, t_query, t_fragment);
    RiString::<S>::try_from(s).expect("resolution result must be a valid IRI")
}

/// Merges the two paths.
///
/// See <https://www.rfc-editor.org/rfc/rfc3986.html#section-5.2.3>.
fn merge(base_path: &str, ref_path: &str, base_authority_defined: bool) -> String {
    if base_authority_defined && base_path.is_empty() {
        format!("/{}", ref_path)
    } else {
        let base_path_end = base_path.rfind('/').map_or(0, |s| s + 1);
        format!("{}{}", &base_path[..base_path_end], ref_path)
    }
}

/// Removes dot segments from the path.
///
/// See <https://www.rfc-editor.org/rfc/rfc3986.html#section-5.2.4>.
fn remove_dot_segments(mut input: String) -> String {
    let mut output = String::new();
    while !input.is_empty() {
        if input.starts_with("../") {
            // 2A.
            input.drain(..3);
        } else if input.starts_with("./") {
            // 2A.
            input.drain(..2);
        } else if input.starts_with("/./") {
            // 2B.
            input.replace_range(..3, "/");
        } else if input == "/." {
            // 2B.
            input.replace_range(..2, "/");
        } else if input.starts_with("/../") {
            // 2C.
            input.replace_range(..4, "/");
            remove_last_segment_and_preceding_slash(&mut output);
        } else if input == "/.." {
            // 2C.
            input.replace_range(..3, "/");
            remove_last_segment_and_preceding_slash(&mut output);
        } else if input == "." {
            // 2D.
            input.drain(..1);
        } else if input == ".." {
            // 2D.
            input.drain(..2);
        } else {
            // 2E.
            let first_seg_end = if let Some(after_slash) = input.strip_prefix('/') {
                // `+1` is the length of the initial slash.
                after_slash
                    .find('/')
                    .map_or_else(|| input.len(), |pos| pos + 1)
            } else {
                input.find('/').unwrap_or(input.len())
            };
            output.extend(input.drain(..first_seg_end));
        }
    }

    output
}

/// Removes the last path segment and the preceding slash if any.
///
/// See <https://www.rfc-editor.org/rfc/rfc3986.html#section-5.2.4>,
/// step 2C.
fn remove_last_segment_and_preceding_slash(output: &mut String) {
    match output.rfind('/') {
        Some(slash_pos) => {
            output.drain(slash_pos..);
        }
        None => output.clear(),
    }
}

/// Recomposes the components.
///
/// See <https://www.rfc-editor.org/rfc/rfc3986.html#section-5.3>.
fn recompose(
    scheme: &str,
    authority: Option<&str>,
    path: &str,
    query: Option<&str>,
    fragment: Option<&str>,
) -> String {
    let mut result = String::new();

    result.push_str(scheme);
    result.push(':');
    if let Some(authority) = authority {
        result.push_str("//");
        result.push_str(authority);
    }
    result.push_str(path);
    if let Some(query) = query {
        result.push('?');
        result.push_str(query);
    }
    if let Some(fragment) = fragment {
        result.push('#');
        result.push_str(fragment);
    }

    result
}
