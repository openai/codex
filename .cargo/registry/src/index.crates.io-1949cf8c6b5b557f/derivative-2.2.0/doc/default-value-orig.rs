pub struct RegexOptions {
    pub pats: Vec<String>,
    pub size_limit: usize,
    pub dfa_size_limit: usize,
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_new_line: bool,
    pub swap_greed: bool,
    pub ignore_whitespace: bool,
    pub unicode: bool,
}

impl Default for RegexOptions {
    fn default() -> Self {
        RegexOptions {
            pats: vec![],
            size_limit: 10 * (1 << 20),
            dfa_size_limit: 2 * (1 << 20),
            case_insensitive: false,
            multi_line: false,
            dot_matches_new_line: false,
            swap_greed: false,
            ignore_whitespace: false,
            unicode: true,
        }
    }
}