//! Numerical accounting for nonnegative rollout-budget units. The same sum is
//! used for one response's hook contributions and for the rollout's posted bills.

/// Accumulate validated, nonnegative `f64` charges without dropping every small
/// contribution after a large one.
#[derive(Clone, Debug, Default)]
pub struct BudgetUnitsSum {
    total: f64,
    remainder: f64,
}

impl BudgetUnitsSum {
    /// Compute the next balance. Invalid inputs and arithmetic overflow leave this sum intact.
    pub fn checked_add(&self, units: f64) -> Option<Self> {
        if !units.is_finite() || units < 0.0 {
            return None;
        }
        if units == 0.0 {
            return Some(self.clone());
        }
        // Near f64::MAX the first addition can overflow before a negative
        // remainder brings it back into range. Halving both inputs avoids that.
        let scale = if (self.total + units).is_finite() {
            1.0
        } else {
            0.5
        };
        let previous = self.total * scale;
        let units = units * scale;
        let sum = previous + units;
        // Recover the small part lost to rounding before adding the prior remainder.
        let lost = if previous >= units {
            (previous - sum) + units
        } else {
            (units - sum) + previous
        };
        let remainder = self.remainder * scale + lost;
        // A nonnegative bill cannot lower posted spending. Keep any unused
        // correction for later receipts rather than reopening the budget.
        let next = (sum + remainder).max(previous);
        let total = next / scale;
        let remainder = ((sum - next) + remainder) / scale;
        (total.is_finite() && remainder.is_finite()).then_some(Self { total, remainder })
    }

    pub fn total(&self) -> f64 {
        self.total
    }
}
