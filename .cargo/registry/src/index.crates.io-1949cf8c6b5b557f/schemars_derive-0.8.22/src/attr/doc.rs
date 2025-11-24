use syn::Attribute;

pub fn get_title_and_desc_from_doc(attrs: &[Attribute]) -> (Option<String>, Option<String>) {
    let doc = match get_doc(attrs) {
        None => return (None, None),
        Some(doc) => doc,
    };

    if doc.starts_with('#') {
        let mut split = doc.splitn(2, '\n');
        let title = split
            .next()
            .unwrap()
            .trim_start_matches('#')
            .trim()
            .to_owned();
        let maybe_desc = split.next().and_then(merge_description_lines);
        (none_if_empty(title), maybe_desc)
    } else {
        (None, merge_description_lines(&doc))
    }
}

fn merge_description_lines(doc: &str) -> Option<String> {
    let desc = doc
        .trim()
        .split("\n\n")
        .filter_map(|line| none_if_empty(line.trim().replace('\n', " ")))
        .collect::<Vec<_>>()
        .join("\n\n");
    none_if_empty(desc)
}

fn get_doc(attrs: &[Attribute]) -> Option<String> {
    let attrs = attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }

            let meta = attr.meta.require_name_value().ok()?;
            if let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(lit_str),
                ..
            }) = &meta.value
            {
                return Some(lit_str.value());
            }

            None
        })
        .collect::<Vec<_>>();

    let mut lines = attrs
        .iter()
        .flat_map(|a| a.split('\n'))
        .map(str::trim)
        .skip_while(|s| s.is_empty())
        .collect::<Vec<_>>();

    if let Some(&"") = lines.last() {
        lines.pop();
    }

    // Added for backward-compatibility, but perhaps we shouldn't do this
    // https://github.com/rust-lang/rust/issues/32088
    if lines.iter().all(|l| l.starts_with('*')) {
        for line in lines.iter_mut() {
            *line = line[1..].trim()
        }
        while let Some(&"") = lines.first() {
            lines.remove(0);
        }
    };

    none_if_empty(lines.join("\n"))
}

fn none_if_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
