//! Checks pool boundaries, snapshot restore, and the v1 sequence through the public API.

use std::collections::BTreeSet;
use std::error::Error;

use codex_utils_model_facing_id::ModelFacingId;
use codex_utils_model_facing_id::ModelFacingIdGenerator;
use codex_utils_model_facing_id::ModelFacingIdGeneratorState;
use codex_utils_model_facing_id::RestoreError;
use pretty_assertions::assert_eq;
use serde_json::json;

/// Every three-digit suffix is issued before the first ID drawn from the six-digit pool.
#[test]
fn exhausts_three_digit_ids_before_opening_the_six_digit_pool() {
    let mut generator = ModelFacingIdGenerator::new(/*seed*/ 42, "environment");
    let mut actual = BTreeSet::new();
    for _ in 0..1_000 {
        actual.insert(generator.fetch_next().to_string());
    }
    let expected: BTreeSet<_> = (0..1_000).map(|id| format!("environment{id:03}")).collect();
    assert_eq!(actual, expected);
    assert_eq!(generator.fetch_next().to_string(), "environment825109");
}

/// IDs can have the same display text and still round-trip with distinct salts and values.
#[test]
fn persisted_ids_keep_the_salt_and_value_separate() -> Result<(), serde_json::Error> {
    for (salt, value) in [("env123", 0), ("env", 123_000)] {
        let saved = json!({ "salt": salt, "value": value });
        let id: ModelFacingId = serde_json::from_value(saved.clone())?;
        assert_eq!(id.to_string(), "env123000");
        assert_eq!(serde_json::to_value(id)?, saved);
    }
    Ok(())
}

/// A saved cursor resumes the uninterrupted sequence both inside and between pools.
#[test]
fn serialized_snapshots_resume_the_sequence() -> Result<(), Box<dyn Error>> {
    let derived_seed = 0x9e6e_7c74_2945_897e;
    for next_ordinal in [437, 1_000, 1_437] {
        let mut original = ModelFacingIdGenerator::new(u64::MAX, "env2");
        for _ in 0..next_ordinal {
            original.fetch_next();
        }
        let snapshot = original.snapshot();
        assert_eq!(
            ModelFacingIdGeneratorState::new(
                /*version*/ 1,
                derived_seed,
                "env2",
                next_ordinal
            ),
            snapshot
        );
        let saved = serde_json::to_value(snapshot)?;
        assert_eq!(
            saved,
            json!({"version": 1, "seed": derived_seed, "salt": "env2", "next_ordinal": next_ordinal})
        );
        let mut resumed = ModelFacingIdGenerator::restore(serde_json::from_value(saved)?)?;
        for _ in 0..1_001 {
            assert_eq!(resumed.fetch_next(), original.fetch_next());
        }
    }
    Ok(())
}

/// V1 must keep the initial outputs and repeated cycle walks stable.
#[test]
fn v1_known_outputs() -> Result<(), Box<dyn Error>> {
    let seed = 0x1234_5678_9abc_def0;
    let state = ModelFacingIdGeneratorState::new(
        /*version*/ 1,
        seed,
        "environment",
        /*next_ordinal*/ 0,
    );
    let mut generator = ModelFacingIdGenerator::restore(state)?;
    for id in ["895", "930", "155", "141", "916", "912", "678", "430"] {
        assert_eq!(
            generator.fetch_next().to_string(),
            format!("environment{id}")
        );
    }
    // Ordinal 1,605 visits two excluded slots before returning to the six-digit pool.
    let state = ModelFacingIdGeneratorState::new(
        /*version*/ 1,
        seed,
        "environment",
        /*next_ordinal*/ 1_605,
    );
    let mut resumed = ModelFacingIdGenerator::restore(state)?;
    assert_eq!(resumed.fetch_next().to_string(), "environment765577");
    Ok(())
}

/// The last ID at each width and the first at the next width stay stable across restores.
#[test]
fn v1_known_outputs_at_every_pool_boundary() -> Result<(), Box<dyn Error>> {
    let seed = 0x1234_5678_9abc_def0;
    for (boundary, last, first) in [
        (1_000, "553", "661326"),
        (1_000_000, "753874", "286136338"),
        (1_000_000_000, "839882170", "813648058018"),
        (1_000_000_000_000, "634462456979", "134726697070558"),
        (
            1_000_000_000_000_000,
            "326088534089890",
            "563366069705075074",
        ),
        (
            1_000_000_000_000_000_000,
            "633936232025849076",
            "012827849051678362461",
        ),
    ] {
        let state = ModelFacingIdGeneratorState::new(/*version*/ 1, seed, "env", boundary - 1);
        let mut generator = ModelFacingIdGenerator::restore(state)?;
        assert_eq!(generator.fetch_next().to_string(), format!("env{last}"));
        let mut resumed = ModelFacingIdGenerator::restore(generator.snapshot())?;
        let next = generator.fetch_next();
        assert_eq!(next.to_string(), format!("env{first}"));
        assert_eq!(resumed.fetch_next(), next);
    }
    Ok(())
}

/// The same session seed can drive several ID kinds, each with its own resumable seed.
#[test]
fn salts_derive_the_seed_stored_in_snapshots() {
    for (seed, salt, derived_seed) in [
        (42, "environment", 0xc9b5_d0ac_9722_ea42),
        (42, "context-window", 0x2747_db8a_6513_485b),
        (43, "environment", 0x70aa_a9fd_4bf2_6e41),
    ] {
        assert_eq!(
            ModelFacingIdGenerator::new(seed, salt).snapshot(),
            ModelFacingIdGeneratorState::new(
                /*version*/ 1,
                derived_seed,
                salt,
                /*next_ordinal*/ 0
            )
        );
    }
}

/// The final ordinal belongs to the last decimal pool. Its successor restarts the sequence.
#[test]
fn wraps_after_the_last_u64_ordinal() -> Result<(), Box<dyn Error>> {
    let seed = 17;
    let last = ModelFacingIdGeneratorState::new(/*version*/ 1, seed, "env", u64::MAX);
    let mut generator = ModelFacingIdGenerator::restore(last)?;
    let text = generator.fetch_next().to_string();
    let last_id = text
        .strip_prefix("env")
        .expect("restored salt")
        .parse::<u64>()?;
    assert!(last_id >= 1_000_000_000_000_000_000);

    let first =
        ModelFacingIdGeneratorState::new(/*version*/ 1, seed, "env", /*next_ordinal*/ 0);
    assert_eq!(generator.snapshot(), first);
    let mut fresh = ModelFacingIdGenerator::restore(first)?;
    let mut resumed = ModelFacingIdGenerator::restore(generator.snapshot())?;
    assert_eq!(resumed.fetch_next(), fresh.fetch_next());
    assert_eq!(resumed.snapshot(), fresh.snapshot());
    Ok(())
}

#[test]
fn rejects_unsupported_snapshot_versions() {
    let saved = ModelFacingIdGeneratorState::new(
        /*version*/ 2, /*seed*/ 0, "env", /*next_ordinal*/ 0,
    );
    assert_eq!(
        ModelFacingIdGenerator::restore(saved).unwrap_err(),
        RestoreError::UnsupportedVersion(2)
    );
}

#[test]
fn rejects_empty_salts_on_restore() {
    let state = ModelFacingIdGeneratorState::new(
        /*version*/ 1, /*seed*/ 42, "", /*next_ordinal*/ 0,
    );
    assert_eq!(
        ModelFacingIdGenerator::restore(state).expect_err("empty salt"),
        RestoreError::InvalidSalt
    );
}
