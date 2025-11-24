use crate::date::{self, Date};
use crate::error::{Error, Result};
use crate::iter::Iter;
use crate::release::{self, Release};
use crate::time;
use crate::version::{Channel::*, Version};
use proc_macro::{Group, TokenTree};
use std::cmp::Ordering;

pub enum Bound {
    Nightly(Date),
    Stable(Release),
}

pub fn parse(paren: Group, iter: Iter) -> Result<Bound> {
    if let Some(TokenTree::Literal(literal)) = iter.peek() {
        let repr = literal.to_string();
        if repr.starts_with(|ch: char| ch.is_ascii_digit()) {
            if repr.contains('.') {
                return release::parse(paren, iter).map(Bound::Stable);
            } else {
                return date::parse(paren, iter).map(Bound::Nightly);
            }
        }
    }
    let msg = format!(
        "expected rustc release number like 1.85, or nightly date like {}",
        time::today(),
    );
    Err(Error::group(paren, msg))
}

impl PartialEq<Bound> for Version {
    fn eq(&self, rhs: &Bound) -> bool {
        match rhs {
            Bound::Nightly(date) => match self.channel {
                Stable | Beta | Dev => false,
                Nightly(nightly) => nightly == *date,
            },
            Bound::Stable(release) => {
                self.minor == release.minor
                    && release.patch.map_or(true, |patch| self.patch == patch)
            }
        }
    }
}

impl PartialOrd<Bound> for Version {
    fn partial_cmp(&self, rhs: &Bound) -> Option<Ordering> {
        match rhs {
            Bound::Nightly(date) => match self.channel {
                Stable | Beta => Some(Ordering::Less),
                Nightly(nightly) => Some(nightly.cmp(date)),
                Dev => Some(Ordering::Greater),
            },
            Bound::Stable(release) => {
                let version = (self.minor, self.patch);
                let bound = (release.minor, release.patch.unwrap_or(0));
                Some(version.cmp(&bound))
            }
        }
    }
}
