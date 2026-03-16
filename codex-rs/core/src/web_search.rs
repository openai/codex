use codex_protocol::models::WebSearchAction;

fn search_action_detail(query: &Option<String>, queries: &Option<Vec<String>>) -> String {
    query.clone().filter(|q| !q.is_empty()).unwrap_or_else(|| {
        let items = queries.as_ref();
        let first = items
            .and_then(|queries| queries.first())
            .cloned()
            .unwrap_or_default();
        if items.is_some_and(|queries| queries.len() > 1) && !first.is_empty() {
            format!("{first} ...")
        } else {
            first
        }
    })
}

pub fn web_search_action_detail(action: &WebSearchAction) -> String {
    match action {
        WebSearchAction::Search { query, queries } => search_action_detail(query, queries),
        WebSearchAction::OpenPage { url } => url.clone().unwrap_or_default(),
        WebSearchAction::FindInPage { url, pattern } => match (pattern, url) {
            (Some(pattern), Some(url)) => format!("'{pattern}' in {url}"),
            (Some(pattern), None) => format!("'{pattern}'"),
            (None, Some(url)) => url.clone(),
            (None, None) => String::new(),
        },
        WebSearchAction::Other => String::new(),
    }
}

pub fn web_search_detail(action: Option<&WebSearchAction>, query: &str) -> String {
    let detail = action.map(web_search_action_detail).unwrap_or_default();
    if detail.is_empty() {
        query.to_string()
    } else {
        detail
    }
}
