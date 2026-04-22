use std::collections::HashMap;

pub fn format_env_display<S: AsRef<str>>(
    env: Option<&HashMap<String, String>>,
    env_vars: &[S],
) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(map) = env {
        let mut pairs: Vec<_> = map.iter().collect();
        pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
        parts.extend(pairs.into_iter().map(|(key, _)| format!("{key}=*****")));
    }

    if !env_vars.is_empty() {
        parts.extend(env_vars.iter().map(|var| format!("{}=*****", var.as_ref())));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_dash_when_empty() {
        let empty_vars: &[String] = &[];
        assert_eq!(format_env_display(/*env*/ None, empty_vars), "-");

        let empty_map = HashMap::new();
        assert_eq!(format_env_display(Some(&empty_map), empty_vars), "-");
    }

    #[test]
    fn formats_sorted_env_pairs() {
        let mut env = HashMap::new();
        env.insert("B".to_string(), "two".to_string());
        env.insert("A".to_string(), "one".to_string());

        assert_eq!(
            format_env_display(Some(&env), &[] as &[String]),
            "A=*****, B=*****"
        );
    }

    #[test]
    fn formats_env_vars_with_dollar_prefix() {
        let vars = vec!["TOKEN".to_string(), "PATH".to_string()];

        assert_eq!(
            format_env_display(/*env*/ None, &vars),
            "TOKEN=*****, PATH=*****"
        );
    }

    #[test]
    fn combines_env_pairs_and_vars() {
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), "/tmp".to_string());
        let vars = vec!["TOKEN".to_string()];

        assert_eq!(
            format_env_display(Some(&env), &vars),
            "HOME=*****, TOKEN=*****"
        );
    }
}
