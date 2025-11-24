use crate::identifier::Identifier;
use crate::{BuildMetadata, Comparator, Prerelease, VersionReq};
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use core::iter::FromIterator;
use core::ops::Deref;

impl Default for Identifier {
    fn default() -> Self {
        Identifier::empty()
    }
}

impl Eq for Identifier {}

impl Hash for Identifier {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.as_str().hash(hasher);
    }
}

impl Deref for Prerelease {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.identifier.as_str()
    }
}

impl Deref for BuildMetadata {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.identifier.as_str()
    }
}

impl PartialOrd for Prerelease {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl PartialOrd for BuildMetadata {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for Prerelease {
    fn cmp(&self, rhs: &Self) -> Ordering {
        if self.identifier.ptr_eq(&rhs.identifier) {
            return Ordering::Equal;
        }

        match self.is_empty() {
            // A real release compares greater than prerelease.
            true => return Ordering::Greater,
            // Prerelease compares less than the real release.
            false if rhs.is_empty() => return Ordering::Less,
            false => {}
        }

        let lhs = self.as_str().split('.');
        let mut rhs = rhs.as_str().split('.');

        for lhs in lhs {
            let rhs = match rhs.next() {
                // Spec: "A larger set of pre-release fields has a higher
                // precedence than a smaller set, if all of the preceding
                // identifiers are equal."
                None => return Ordering::Greater,
                Some(rhs) => rhs,
            };

            let string_cmp = || Ord::cmp(lhs, rhs);
            let is_ascii_digit = |b: u8| b.is_ascii_digit();
            let ordering = match (
                lhs.bytes().all(is_ascii_digit),
                rhs.bytes().all(is_ascii_digit),
            ) {
                // Respect numeric ordering, for example 99 < 100. Spec says:
                // "Identifiers consisting of only digits are compared
                // numerically."
                (true, true) => Ord::cmp(&lhs.len(), &rhs.len()).then_with(string_cmp),
                // Spec: "Numeric identifiers always have lower precedence than
                // non-numeric identifiers."
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                // Spec: "Identifiers with letters or hyphens are compared
                // lexically in ASCII sort order."
                (false, false) => string_cmp(),
            };

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        if rhs.next().is_none() {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    }
}

impl Ord for BuildMetadata {
    fn cmp(&self, rhs: &Self) -> Ordering {
        if self.identifier.ptr_eq(&rhs.identifier) {
            return Ordering::Equal;
        }

        let lhs = self.as_str().split('.');
        let mut rhs = rhs.as_str().split('.');

        for lhs in lhs {
            let rhs = match rhs.next() {
                None => return Ordering::Greater,
                Some(rhs) => rhs,
            };

            let is_ascii_digit = |b: u8| b.is_ascii_digit();
            let ordering = match (
                lhs.bytes().all(is_ascii_digit),
                rhs.bytes().all(is_ascii_digit),
            ) {
                (true, true) => {
                    // 0 < 00 < 1 < 01 < 001 < 2 < 02 < 002 < 10
                    let lhval = lhs.trim_start_matches('0');
                    let rhval = rhs.trim_start_matches('0');
                    Ord::cmp(&lhval.len(), &rhval.len())
                        .then_with(|| Ord::cmp(lhval, rhval))
                        .then_with(|| Ord::cmp(&lhs.len(), &rhs.len()))
                }
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                (false, false) => Ord::cmp(lhs, rhs),
            };

            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        if rhs.next().is_none() {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    }
}

impl FromIterator<Comparator> for VersionReq {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Comparator>,
    {
        let comparators = Vec::from_iter(iter);
        VersionReq { comparators }
    }
}
