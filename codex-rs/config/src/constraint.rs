use std::fmt;
use std::sync::Arc;

use crate::config_loader::RequirementSource;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConstraintError {
    #[error(
        "invalid value for `{field_name}`: `{candidate}` is not in the allowed set {allowed} (set by {requirement_source})"
    )]
    InvalidValue {
        field_name: &'static str,
        candidate: String,
        allowed: String,
        requirement_source: RequirementSource,
    },

    #[error("field `{field_name}` cannot be empty")]
    EmptyField { field_name: String },

    #[error("invalid rules in requirements (set by {requirement_source}): {reason}")]
    ExecPolicyParse {
        requirement_source: RequirementSource,
        reason: String,
    },
}

impl ConstraintError {
    pub fn empty_field(field_name: impl Into<String>) -> Self {
        Self::EmptyField {
            field_name: field_name.into(),
        }
    }
}

pub type ConstraintResult<T> = Result<T, ConstraintError>;

impl From<ConstraintError> for std::io::Error {
    fn from(err: ConstraintError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, err)
    }
}

type ConstraintValidator<T> = dyn Fn(&T) -> ConstraintResult<()> + Send + Sync;
/// A ConstraintNormalizer is a function which transforms a value into another of the same type.
/// `Constrained` uses normalizers to transform values to satisfy constraints or enforce values.
type ConstraintNormalizer<T> = dyn Fn(T) -> T + Send + Sync;

#[derive(Clone)]
pub struct Constrained<T> {
    value: T,
    validator: Arc<ConstraintValidator<T>>,
    normalizer: Option<Arc<ConstraintNormalizer<T>>>,
}

impl<T: Send + Sync> Constrained<T> {
    pub fn new(
        initial_value: T,
        validator: impl Fn(&T) -> ConstraintResult<()> + Send + Sync + 'static,
    ) -> ConstraintResult<Self> {
        let validator: Arc<ConstraintValidator<T>> = Arc::new(validator);
        validator(&initial_value)?;
        Ok(Self {
            value: initial_value,
            validator,
            normalizer: None,
        })
    }

    /// normalized creates a `Constrained` value with a normalizer function and a validator that allows any value.
    pub fn normalized(
        initial_value: T,
        normalizer: impl Fn(T) -> T + Send + Sync + 'static,
    ) -> ConstraintResult<Self> {
        let validator: Arc<ConstraintValidator<T>> = Arc::new(|_| Ok(()));
        let normalizer: Arc<ConstraintNormalizer<T>> = Arc::new(normalizer);
        let normalized = normalizer(initial_value);
        validator(&normalized)?;
        Ok(Self {
            value: normalized,
            validator,
            normalizer: Some(normalizer),
        })
    }

    pub fn allow_any(initial_value: T) -> Self {
        Self {
            value: initial_value,
            validator: Arc::new(|_| Ok(())),
            normalizer: None,
        }
    }

    /// Allow any value of T, using T's Default as the initial value.
    pub fn allow_any_from_default() -> Self
    where
        T: Default,
    {
        Self::allow_any(T::default())
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn value(&self) -> T
    where
        T: Copy,
    {
        self.value
    }

    pub fn can_set(&self, candidate: &T) -> ConstraintResult<()> {
        (self.validator)(candidate)
    }

    pub fn set(&mut self, value: T) -> ConstraintResult<()> {
        let value = if let Some(normalizer) = &self.normalizer {
            normalizer(value)
        } else {
            value
        };
        (self.validator)(&value)?;
        self.value = value;
        Ok(())
    }
}

impl<T> std::ops::Deref for Constrained<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for Constrained<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Constrained")
            .field("value", &self.value)
            .finish()
    }
}

impl<T: PartialEq> PartialEq for Constrained<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn invalid_value(candidate: impl Into<String>, allowed: impl Into<String>) -> ConstraintError {
        ConstraintError::InvalidValue {
            field_name: "<unknown>",
            candidate: candidate.into(),
            allowed: allowed.into(),
            requirement_source: RequirementSource::Unknown,
        }
    }

    #[test]
    fn constrained_allow_any_accepts_any_value() {
        let mut constrained = Constrained::allow_any(5);
        constrained.set(-10).expect("allow any accepts all values");
        assert_eq!(constrained.value(), -10);
    }

    #[test]
    fn constrained_allow_any_default_uses_default_value() {
        let constrained = Constrained::<i32>::allow_any_from_default();
        assert_eq!(constrained.value(), 0);
    }

    #[test]
    fn constrained_normalizer_applies_on_init_and_set() -> anyhow::Result<()> {
        let mut constrained = Constrained::normalized(-1, |value| value.max(0))?;
        assert_eq!(constrained.value(), 0);
        constrained.set(-5)?;
        assert_eq!(constrained.value(), 0);
        constrained.set(10)?;
        assert_eq!(constrained.value(), 10);
        Ok(())
    }

    #[test]
    fn constrained_new_rejects_invalid_initial_value() {
        let result = Constrained::new(0, |value| {
            if *value > 0 {
                Ok(())
            } else {
                Err(invalid_value(value.to_string(), "positive values"))
            }
        });

        assert_eq!(result, Err(invalid_value("0", "positive values")));
    }

    #[test]
    fn constrained_set_rejects_invalid_value_and_leaves_previous() {
        let mut constrained = Constrained::new(1, |value| {
            if *value > 0 {
                Ok(())
            } else {
                Err(invalid_value(value.to_string(), "positive values"))
            }
        })
        .expect("initial value should be accepted");

        let err = constrained
            .set(-5)
            .expect_err("negative values should be rejected");
        assert_eq!(err, invalid_value("-5", "positive values"));
        assert_eq!(constrained.value(), 1);
    }

    #[test]
    fn constrained_can_set_allows_probe_without_setting() {
        let constrained = Constrained::new(1, |value| {
            if *value > 0 {
                Ok(())
            } else {
                Err(invalid_value(value.to_string(), "positive values"))
            }
        })
        .expect("initial value should be accepted");

        constrained
            .can_set(&2)
            .expect("can_set should accept positive value");
        let err = constrained
            .can_set(&-1)
            .expect_err("can_set should reject negative value");
        assert_eq!(err, invalid_value("-1", "positive values"));
        assert_eq!(constrained.value(), 1);
    }
}
