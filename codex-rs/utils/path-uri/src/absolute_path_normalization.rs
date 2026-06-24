use crate::PathConvention;
use crate::PathUri;
use url::Url;

pub(super) fn path_uri_from_segments<'a>(
    convention: PathConvention,
    host: Option<&str>,
    segments: impl Iterator<Item = &'a str>,
) -> Option<PathUri> {
    let mut url = Url::parse("file:///").ok()?;
    if let Some(host) = host {
        url.set_host(Some(host)).ok()?;
    }
    let anchor_depth = usize::from(convention == PathConvention::Windows);
    let mut depth = 0;
    let mut normalized_segments = Vec::new();
    let mut has_trailing_separator = false;
    for segment in segments {
        match segment {
            "" => has_trailing_separator = true,
            "." => has_trailing_separator = false,
            ".." => {
                has_trailing_separator = false;
                if depth > anchor_depth {
                    normalized_segments.pop();
                    depth -= 1;
                }
            }
            segment => {
                normalized_segments.push(segment);
                depth += 1;
                has_trailing_separator = false;
            }
        }
    }
    if has_trailing_separator
        || (convention == PathConvention::Windows && host.is_none() && depth == anchor_depth)
    {
        normalized_segments.push("");
    }
    {
        let mut url_segments = url.path_segments_mut().ok()?;
        url_segments.clear().extend(normalized_segments);
    }
    PathUri::try_from(url).ok()
}
