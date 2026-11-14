//! Numerical accounting for nonnegative rollout-budget units. The same sum is
//! used for one response's hook contributions and for the rollout's posted bills.

/// Accumulate validated, nonnegative `f64` charges without dropping every small
/// contribution after a large one. An unrepresentable total stays infinite.
#[derive(Debug, Default)]
pub struct BudgetUnitsSum {
    total: f64,
    remainder: f64,
}

impl BudgetUnitsSum {
    pub fn add(&mut self, units: f64) {
        debug_assert!(units >= 0.0);
        if units == 0.0 || self.total.is_infinite() {
            return;
        }
        if units.is_infinite() {
            self.total = f64::INFINITY;
            self.remainder = 0.0;
            return;
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
        self.total = next / scale;
        self.remainder = if self.total.is_finite() {
            ((sum - next) + remainder) / scale
        } else {
            0.0
        };
    }

    pub fn total(&self) -> f64 {
        self.total
    }
}
