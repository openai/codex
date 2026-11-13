//! Issues a salt followed by shuffled decimal IDs from `000`–`999`, then `001000`–`999999`.
//! Each pool uses a four-round Feistel permutation with BLAKE3 and cycle-walking.
//! The seed can be varied across sessions to change the order in which IDs are issued.

use std::borrow::Cow;
use std::fmt;
use std::ops::RangeInclusive;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

mod bindings;
mod environment;
pub use bindings::ModelIdKey;
pub use bindings::ModelIdKind;
pub use bindings::ModelIdMap;
pub use environment::ModelEnvironmentId;
pub use environment::RegisteredEnvironmentId;

const VERSION: u32 = 1;
// Our tokenizers encode every decimal ID of up to three digits as a single token.
const GROUP_SIZE: u64 = 1_000;

/// Stores the generator salt, allocation ordinal and shuffled value.
/// Display pads the value to groups of three digits.
/// With salt `environment`, zero renders as `environment000` and 1000 as `environment001000`.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct ModelFacingId {
    salt: Cow<'static, str>,
    /// Zero-based position in the generator sequence.
    ordinal: u64,
    value: u64,
}

impl fmt::Display for ModelFacingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.salt, self.digits())
    }
}

impl ModelFacingId {
    /// Identifies the generator sequence and prefixes the displayed number.
    pub fn salt(&self) -> &str {
        &self.salt
    }

    /// Position in the allocation sequence before shuffling.
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Formats the padded digits for protocols that supply their own type prefix.
    pub fn digits(&self) -> impl fmt::Display + '_ {
        padded_digits(self.value)
    }
}

fn padded_digits(value: u64) -> impl fmt::Display {
    fmt::from_fn(move |f| write!(f, "{value:0width$}", width = decimal_width(value)))
}

fn decimal_width(value: u64) -> usize {
    (value.checked_ilog10().unwrap_or(0) as usize + 1).next_multiple_of(3)
}

/// Generates token-efficient identifiers for model context.
#[derive(Debug)]
pub struct ModelFacingIdGenerator {
    state: ModelFacingIdGeneratorState,
}

impl ModelFacingIdGenerator {
    /// Starts a sequence for one ID kind using a shared session seed.
    /// The salt also prefixes each displayed ID. Use the [tokenizer](https://platform.openai.com/tokenizer)
    /// to choose a stable, single-token salt such as `environment`.
    ///
    /// # Panics
    ///
    /// Panics if the salt is empty.
    pub fn new(seed: u64, salt: &'static str) -> Self {
        Self::starting_at(seed, salt, /*next_ordinal*/ 0)
    }

    /// Continues a sequence at the given ordinal using its original seed and salt.
    pub fn starting_at(seed: u64, salt: &'static str, next_ordinal: u64) -> Self {
        assert!(!salt.is_empty(), "a model ID salt must be nonempty");
        let mut hasher = blake3::Hasher::new();
        hasher.update(&seed.to_le_bytes());
        hasher.update(salt.as_bytes());
        let mut seed_bytes = [0; 8];
        hasher.finalize_xof().fill(&mut seed_bytes);
        let derived_seed = u64::from_le_bytes(seed_bytes);
        Self {
            state: ModelFacingIdGeneratorState::new(VERSION, derived_seed, salt, next_ordinal),
        }
    }

    /// Issues the next ID. After all `u64` values are issued, the sequence repeats from zero.
    pub fn fetch_next(&mut self) -> ModelFacingId {
        let ordinal = self.state.next_ordinal;
        let mut pool = 0..=GROUP_SIZE - 1;
        while ordinal > *pool.end() {
            let start = pool.end() + 1;
            // The final decimal pool ends at u64::MAX, which has no representable successor.
            let last = start
                .checked_mul(GROUP_SIZE)
                .map_or(u64::MAX, |end| end - 1);
            pool = start..=last;
        }
        let id = permute(self.state.seed, pool, ordinal);
        let (next, wrapped) = ordinal.overflowing_add(1);
        if wrapped {
            tracing::error!(
                "model-facing ID generator exhausted all u64 values and wrapped to zero"
            );
        }
        self.state.next_ordinal = next;
        ModelFacingId {
            salt: self.state.salt.clone(),
            ordinal,
            value: id,
        }
    }

    /// Position that the next allocation will use.
    pub fn next_ordinal(&self) -> u64 {
        self.state.next_ordinal
    }

    /// Captures the position of the next allocation.
    pub fn snapshot(&self) -> ModelFacingIdGeneratorState {
        self.state.clone()
    }

    /// Continues the snapshot's sequence from its next unissued ordinal.
    pub fn restore(state: ModelFacingIdGeneratorState) -> Result<Self, RestoreError> {
        if state.version != VERSION {
            return Err(RestoreError::UnsupportedVersion(state.version));
        }
        if state.salt.is_empty() {
            return Err(RestoreError::InvalidSalt);
        }
        Ok(Self { state })
    }
}

/// Persisted sequence version, derived seed, salt, and zero-based next allocation ordinal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModelFacingIdGeneratorState {
    version: u32,
    seed: u64,
    salt: Cow<'static, str>,
    next_ordinal: u64,
}

impl ModelFacingIdGeneratorState {
    /// Recovers the ID at a stored binding's ordinal in this version of the sequence.
    pub fn id_at(&self, ordinal: u64) -> Result<ModelFacingId, RestoreError> {
        let mut state = self.clone();
        state.next_ordinal = ordinal;
        Ok(ModelFacingIdGenerator::restore(state)?.fetch_next())
    }

    /// Returns the namespace this snapshot belongs to.
    pub fn salt(&self) -> &str {
        &self.salt
    }

    /// Rebuilds a snapshot from its stored fields. [`ModelFacingIdGenerator::restore`] validates it.
    pub fn new(
        version: u32,
        seed: u64,
        salt: impl Into<Cow<'static, str>>,
        next_ordinal: u64,
    ) -> Self {
        Self {
            version,
            seed,
            salt: salt.into(),
            next_ordinal,
        }
    }
}

/// The snapshot cannot be used to resume an allocation sequence.
#[derive(Debug, PartialEq, Eq, Error)]
pub enum RestoreError {
    #[error("unsupported model-facing ID generator version {0}")]
    UnsupportedVersion(u32),
    #[error("a model ID salt must be nonempty")]
    InvalidSalt,
}

// Use the smallest balanced Feistel domain that covers the pool to limit cycle-walking.
// Both the four rounds and their input encoding are part of the snapshot format.
fn permute(seed: u64, pool: RangeInclusive<u64>, ordinal: u64) -> u64 {
    let start = *pool.start();
    let size = pool.end() - start + 1;
    let bits = u64::BITS - (size - 1).leading_zeros();
    let half_bits = bits.div_ceil(2);
    let half_mask = (1_u64 << half_bits) - 1;
    // The seed, pool index, and round occupy 17 bytes. The pool width fixes the right half's length.
    let input_len = 17 + half_bits.div_ceil(8) as usize;
    let mut input = [0; 25];
    input[..8].copy_from_slice(&seed.to_le_bytes());
    input[8..16].copy_from_slice(&(start / GROUP_SIZE).to_le_bytes());

    let mut slot = ordinal - start;
    loop {
        let (mut left, mut right) = (slot >> half_bits, slot & half_mask);
        for round in 0..4 {
            input[16] = round;
            input[17..].copy_from_slice(&right.to_le_bytes());
            let hash = blake3::hash(&input[..input_len]);
            let mut bytes = [0; 8];
            bytes.copy_from_slice(&hash.as_bytes()[..8]);
            let f = u64::from_le_bytes(bytes) & half_mask;
            (left, right) = (right, left ^ f);
        }
        slot = (left << half_bits) | right;
        // Following the permutation cycle must eventually return to the starting slot.
        if slot < size {
            return start + slot;
        }
    }
}
