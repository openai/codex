use codex_tui::ComposerInput;

pub struct NewTaskPage {
    pub composer: ComposerInput,
    pub submitting: bool,
    pub env_id: Option<String>,
    pub best_of_n: usize,
}

impl NewTaskPage {
    pub fn new(env_id: Option<String>, best_of_n: usize) -> Self {
        let mut composer = ComposerInput::new();
        composer.set_hint_items(vec![
            ("⏎", "send"),
            ("Shift+⏎", "newline"),
            ("Ctrl+O", "env"),
            ("Ctrl+N", "attempts"),
            ("Ctrl+C", "quit"),
        ]);
        Self {
            composer,
            submitting: false,
            env_id,
            best_of_n,
        }
    }

    // Additional helpers can be added as usage evolves.
}

impl Default for NewTaskPage {
    fn default() -> Self {
        Self::new(None, 1)
    }
}
