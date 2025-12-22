use std::fmt;
use std::sync::Arc;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConstraintError {
    #[error("value `{candidate}` is not in the allowed set {allowed}")]
    InvalidValue { candidate: String, allowed: String },

    #[error("field `{field_name}` cannot be empty")]
    EmptyField { field_name: String },
}

impl ConstraintError {
    pub fn invalid_value(candidate: impl Into<String>, allowed: impl Into<String>) -> Self {
        Self::InvalidValue {
            candidate: candidate.into(),
            allowed: allowed.into(),
        }
    }

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

#[derive(Clone)]
pub struct Constrained<T> {
    value: T,
    validator: Arc<ConstraintValidator<T>>,
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
        })
    }

    pub fn allow_any(initial_value: T) -> Self {
        Self {
            value: initial_value,
            validator: Arc::new(|_| Ok(())),
        }
    }

    pub fn allow_only(value: T) -> Self
    where
        T: PartialEq + Send + Sync + fmt::Debug + Clone + 'static,
    {
        #[expect(clippy::expect_used)]
        Self::new(value.clone(), move |candidate| {
            if *candidate == value {
                Ok(())
            } else {
                Err(ConstraintError::invalid_value(
                    format!("{candidate:?}"),
                    format!("{value:?}"),
                ))
            }
        })
        .expect("initial value should always be valid")
    }

    /// Allow any value of T, using T's Default as the initial value.
    pub fn allow_any_from_default() -> Self
    where
        T: Default,
    {
        Self::allow_any(T::default())
    }

    pub fn allow_values(initial_value: T, allowed: Vec<T>) -> ConstraintResult<Self>
    where
        T: PartialEq + Send + Sync + fmt::Debug + 'static,
    {
        Self::new(initial_value, move |candidate| {
            if allowed.contains(candidate) {
                Ok(())
            } else {
                Err(ConstraintError::invalid_value(
                    format!("{candidate:?}"),
                    format!("{allowed:?}"),
                ))
            }
        })
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
    fn constrained_new_rejects_invalid_initial_value() {
        let result = Constrained::new(0, |value| {
            if *value > 0 {
                Ok(())
            } else {
                Err(ConstraintError::invalid_value(
                    value.to_string(),
                    "positive values",
                ))
            }
        });

        assert_eq!(
            result,
            Err(ConstraintError::invalid_value("0", "positive values"))
        );
    }

    #[test]
    fn constrained_set_rejects_invalid_value_and_leaves_previous() {
        let mut constrained = Constrained::new(1, |value| {
            if *value > 0 {
                Ok(())
            } else {
                Err(ConstraintError::invalid_value(
                    value.to_string(),
                    "positive values",
                ))
            }
        })
        .expect("initial value should be accepted");

        let err = constrained
            .set(-5)
            .expect_err("negative values should be rejected");
        assert_eq!(err, ConstraintError::invalid_value("-5", "positive values"));
        assert_eq!(constrained.value(), 1);
    }

    #[test]
    fn constrained_can_set_allows_probe_without_setting() {
        let constrained = Constrained::new(1, |value| {
            if *value > 0 {
                Ok(())
            } else {
                Err(ConstraintError::invalid_value(
                    value.to_string(),
                    "positive values",
                ))
            }
        })
        .expect("initial value should be accepted");

        constrained
            .can_set(&2)
            .expect("can_set should accept positive value");
        let err = constrained
            .can_set(&-1)
            .expect_err("can_set should reject negative value");
        assert_eq!(err, ConstraintError::invalid_value("-1", "positive values"));
        assert_eq!(constrained.value(), 1);
    }
}
