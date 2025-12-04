use rand::{
    distributions::Alphanumeric,
    prelude::{Rng, SeedableRng, StdRng},
};

const OPERATORS: &[char] = &[
    '+', '-', '<', '>', '(', ')', '*', '/', '&', '|', '!', ',', '.', '%',
];

pub struct Rand(StdRng);

impl Rand {
    #[must_use]
    pub fn new(seed: usize) -> Self {
        Self(StdRng::seed_from_u64(seed as u64))
    }

    pub fn unsigned(&mut self, max: usize) -> usize {
        self.0.gen_range(0..=max)
    }

    pub fn words(&mut self, max_count: usize) -> Vec<u8> {
        let word_count = self.unsigned(max_count);
        let mut result = Vec::with_capacity(2 * word_count);
        for i in 0..word_count {
            if i > 0 {
                if self.unsigned(5) == 0 {
                    result.push(b'\n');
                } else {
                    result.push(b' ');
                }
            }
            if self.unsigned(3) == 0 {
                let index = self.unsigned(OPERATORS.len() - 1);
                result.push(OPERATORS[index] as u8);
            } else {
                for _ in 0..self.unsigned(8) {
                    result.push(self.0.sample(Alphanumeric));
                }
            }
        }
        result
    }
}
