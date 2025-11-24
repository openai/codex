#![allow(dead_code)]

use crate::date::Date;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Version {
    pub minor: u16,
    pub patch: u16,
    pub channel: Channel,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Channel {
    Stable,
    Beta,
    Nightly(Date),
    Dev,
}
