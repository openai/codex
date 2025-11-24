# extern crate derivative;
# use derivative::Derivative;
#[derive(Derivative)]
#[derivative(Default)]
pub struct RegexOptions {
    pub pats: Vec<String>,
    #[derivative(Default(value="10 * (1 << 20)"))]
    pub size_limit: usize,
    #[derivative(Default(value="2 * (1 << 20)"))]
    pub dfa_size_limit: usize,
    pub case_insensitive: bool,
    pub multi_line: bool,
    pub dot_matches_new_line: bool,
    pub swap_greed: bool,
    pub ignore_whitespace: bool,
    #[derivative(Default(value="true"))]
    pub unicode: bool,
}