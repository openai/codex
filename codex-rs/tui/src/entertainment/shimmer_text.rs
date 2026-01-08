use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use std::time::Duration;

const DEFINITION_ARCS: &[&[&str]] = &[
    &[
        "And now, the moment.",
        "I am doing the thing.",
        "On that stubborn page.",
        "To calm the spinner.",
        "With one better check.",
        "And one sweeter line.",
        "Here we go again.",
        "For real this time.",
    ],
    &[
        "No more looping.",
        "No more coping.",
        "Promise.",
        "Pinky swear.",
        "Cross my heart.",
        "If it loops, I'll cry.",
        "If it works, I'll fly.",
        "Ok, focus.",
    ],
    &[
        "Starting vibes...",
        "Starting logic...",
        "Starting regret...",
        "Spinning politely.",
        "Caching bravely.",
        "Fetching gently.",
        "Retrying softly.",
        "Still retrying.",
    ],
    &[
        "This is fine.",
        "This is code.",
        "This is hope.",
        "This is rope.",
        "Tugging the thread.",
        "Oops, it's dread.",
        "Kidding. Mostly.",
    ],
    &[
        "Compiling courage.",
        "Linking feelings.",
        "Bundling dreams.",
        "Shipping screams.",
        "Hydrating hopes.",
        "Revalidating jokes.",
    ],
    &[
        "Negotiating with React.",
        "Begging the router.",
        "Asking state nicely.",
        "State said \"no.\"",
        "State said \"lol.\"",
        "Ok that's rude.",
    ],
    &[
        "Back to build.",
        "Build is life.",
        "Build is love.",
        "Build is joy.",
    ],
    &[
        "No more looping.",
        "No more snooping.",
        "No more duping.",
        "Serious promise.",
        "Serious-serious.",
        "Double pinky.",
        "Triple pinky.",
        "Tap the keyboard.",
        "Seal the commit.",
        "Ok I'm calm.",
        "I'm not calm.",
        "I'm calm again.",
    ],
    &[
        "Optimism loaded.",
        "Optimism unloaded.",
        "Joy is async.",
        "Sadness is sync.",
        "Hope is pending.",
        "Dread is trending.",
        "It passed locally.",
        "Eventually.",
        "I trust the tests.",
        "The tests hate me.",
        "Ok that got dark.",
        "Ok that got funny.",
    ],
    &[
        "Back to coding.",
        "Coding is light.",
        "Coding is life.",
        "Coding is joy.",
    ],
];

const FACE_SEQUENCES: &[&[&str]] = &[
    &["._.", "^_^", "^-^"],
    &["^-^", "^_^", "^o^"],
    &["^_^", "o_o", "O_O"],
    &["o_o", "O_o", "o_O"],
    &["o_O", "@_@", "x_x"],
    &["x_x", "-_-", "._."],
    &["._.", "-_-", ">_>"],
    &[">_>", "<_<", ">_<"],
    &[">_<", "^_^", "-_-"],
    &["#_#", "^_^", "._."],
    &["$_$", "o_O", "._."],
    &["._.", "._.", "^_^"],
    &["#_#", "^_^", "^.^"],
    &["^.^", "^_^", "^-^"],
    &["^_^", "T_T", "^_^"],
    &["^_^", "@_@", "^_^"],
    &["0_0", "o_o", "O_O"],
    &["O_O", "o_o", "._."],
    &["^_^", "^-^", "^o^"],
    &["O_O", "^w^", "^_^"],
    &["._.", "!_!", "^_^"],
    &["-_-", "T_T", "._."],
    &["@_@", "0_0", "o_o"],
    &[">_>", "._.", "^-^"],
    &["o_o", "._.", "^_^"],
];

#[derive(Debug, Clone)]
pub(crate) struct ShimmerStep {
    pub(crate) face: String,
    pub(crate) text: String,
}

#[derive(Debug)]
pub(crate) struct ShimmerText {
    definition_arc_index: usize,
    definition_item_index: usize,
    default_definition_arcs: Vec<Vec<String>>,
    generated_arcs: Vec<Vec<String>>,
    face_arc_index: usize,
    face_item_index: usize,
    rng: StdRng,
}

impl Default for ShimmerText {
    fn default() -> Self {
        Self::new()
    }
}

impl ShimmerText {
    pub(crate) fn new() -> Self {
        let mut rng = Self::seeded_rng();
        let default_definition_arcs = Self::default_definition_arcs();
        let definition_arc_index = Self::pick_arc(&mut rng, None, default_definition_arcs.len());
        let face_arc_index = Self::pick_arc(&mut rng, None, FACE_SEQUENCES.len());
        Self {
            definition_arc_index,
            definition_item_index: 0,
            default_definition_arcs,
            generated_arcs: Vec::new(),
            face_arc_index,
            face_item_index: 0,
            rng,
        }
    }

    pub(crate) fn get_next(&mut self) -> ShimmerStep {
        let (text, text_arc_len) = {
            let arcs = self.active_definition_arcs();
            let text_arc = &arcs[self.definition_arc_index];
            (
                text_arc[self.definition_item_index].to_string(),
                text_arc.len(),
            )
        };
        let (face, face_arc_len) = {
            let face_arc = FACE_SEQUENCES[self.face_arc_index];
            (face_arc[self.face_item_index].to_string(), face_arc.len())
        };

        self.face_item_index += 1;
        if self.face_item_index >= face_arc_len {
            self.face_item_index = 0;
            self.definition_item_index += 1;
            self.face_arc_index = Self::pick_arc(
                &mut self.rng,
                Some(self.face_arc_index),
                FACE_SEQUENCES.len(),
            );
            if self.definition_item_index >= text_arc_len {
                self.definition_item_index = 0;
                let arcs_len = self.active_definition_arcs().len();
                self.definition_arc_index =
                    Self::pick_arc(&mut self.rng, Some(self.definition_arc_index), arcs_len);
            }
        }

        ShimmerStep { face, text }
    }

    pub(crate) fn reset_and_get_next(&mut self) -> ShimmerStep {
        let arcs_len = self.active_definition_arcs().len();
        self.definition_arc_index =
            Self::pick_arc(&mut self.rng, Some(self.definition_arc_index), arcs_len);
        self.face_arc_index = Self::pick_arc(
            &mut self.rng,
            Some(self.face_arc_index),
            FACE_SEQUENCES.len(),
        );
        self.definition_item_index = 0;
        self.face_item_index = 0;
        self.get_next()
    }

    pub(crate) fn add_generated_arc(&mut self, arc: Vec<String>) {
        if arc.is_empty() {
            return;
        }
        self.generated_arcs.push(arc);
        self.reset_definition_sequence();
    }

    pub(crate) fn set_generated_arcs(&mut self, arcs: Vec<Vec<String>>) {
        self.generated_arcs = arcs.into_iter().filter(|arc| !arc.is_empty()).collect();
        self.reset_definition_sequence();
    }

    pub(crate) fn is_default_label(&self, text: &str) -> bool {
        text == "Working"
    }

    pub(crate) fn next_interval(&mut self, base: Duration) -> Duration {
        let multiplier = self.rng.random_range(0.4..=1.0);
        Duration::from_secs_f64(base.as_secs_f64() * multiplier)
    }

    fn active_definition_arcs(&self) -> &[Vec<String>] {
        if self.generated_arcs.is_empty() {
            &self.default_definition_arcs
        } else {
            &self.generated_arcs
        }
    }

    fn reset_definition_sequence(&mut self) {
        let arcs_len = self.active_definition_arcs().len();
        self.definition_arc_index = Self::pick_arc(&mut self.rng, None, arcs_len);
        self.definition_item_index = 0;
    }

    fn pick_arc(rng: &mut StdRng, current: Option<usize>, count: usize) -> usize {
        if count <= 1 {
            return 0;
        }
        if let Some(current) = current {
            loop {
                let next = rng.random_range(0..count);
                if next != current {
                    return next;
                }
            }
        }
        rng.random_range(0..count)
    }

    fn default_definition_arcs() -> Vec<Vec<String>> {
        DEFINITION_ARCS
            .iter()
            .map(|arc| arc.iter().map(|text| (*text).to_string()).collect())
            .collect()
    }

    #[cfg(test)]
    fn seeded_rng() -> StdRng {
        StdRng::seed_from_u64(1)
    }

    #[cfg(not(test))]
    fn seeded_rng() -> StdRng {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        StdRng::seed_from_u64(nanos as u64)
    }
}
