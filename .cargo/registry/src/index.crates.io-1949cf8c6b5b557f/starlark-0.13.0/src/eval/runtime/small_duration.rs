/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::iter::Sum;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Div;
use std::time::Duration;

use allocative::Allocative;
use dupe::Dupe;

/// Slightly faster than `Duration`.
#[derive(
    Copy, Clone, Dupe, Default, Eq, PartialEq, Ord, PartialOrd, Debug, Allocative
)]
pub(crate) struct SmallDuration {
    /// `u64::MAX` nanos is 500 years.
    pub(crate) nanos: u64,
}

impl SmallDuration {
    pub(crate) fn from_duration(duration: Duration) -> SmallDuration {
        SmallDuration {
            nanos: duration.as_nanos() as u64,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_millis(millis: u64) -> SmallDuration {
        Self::from_duration(Duration::from_millis(millis))
    }

    pub(crate) fn to_duration(self) -> Duration {
        Duration::from_nanos(self.nanos)
    }
}

impl AddAssign for SmallDuration {
    fn add_assign(&mut self, other: Self) {
        self.nanos += other.nanos;
    }
}

impl AddAssign<Duration> for SmallDuration {
    fn add_assign(&mut self, other: Duration) {
        self.nanos += other.as_nanos() as u64;
    }
}

impl Add<Duration> for SmallDuration {
    type Output = SmallDuration;

    fn add(self, other: Duration) -> Self::Output {
        SmallDuration {
            nanos: self.nanos + other.as_nanos() as u64,
        }
    }
}

impl Add<SmallDuration> for SmallDuration {
    type Output = SmallDuration;

    fn add(self, other: SmallDuration) -> SmallDuration {
        SmallDuration {
            nanos: self.nanos + other.nanos,
        }
    }
}

impl Div<u64> for SmallDuration {
    type Output = SmallDuration;

    fn div(self, other: u64) -> SmallDuration {
        SmallDuration {
            nanos: self.nanos / other,
        }
    }
}

impl<'a> Sum<&'a SmallDuration> for SmallDuration {
    fn sum<I>(iter: I) -> SmallDuration
    where
        I: Iterator<Item = &'a SmallDuration>,
    {
        iter.fold(SmallDuration::default(), |acc, x| acc + *x)
    }
}

impl Sum<SmallDuration> for SmallDuration {
    fn sum<I>(iter: I) -> SmallDuration
    where
        I: Iterator<Item = SmallDuration>,
    {
        iter.fold(SmallDuration::default(), |acc, x| acc + x)
    }
}
