use crate::symbols::{block, line};

/// Scrollbar Set
/// ```text
/// <--▮------->
/// ^  ^   ^   ^
/// │  │   │   └ end
/// │  │   └──── track
/// │  └──────── thumb
/// └─────────── begin
/// ```
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Set<'a> {
    pub track: &'a str,
    pub thumb: &'a str,
    pub begin: &'a str,
    pub end: &'a str,
}

pub const DOUBLE_VERTICAL: Set = Set {
    track: line::DOUBLE_VERTICAL,
    thumb: block::FULL,
    begin: "▲",
    end: "▼",
};

pub const DOUBLE_HORIZONTAL: Set = Set {
    track: line::DOUBLE_HORIZONTAL,
    thumb: block::FULL,
    begin: "◄",
    end: "►",
};

pub const VERTICAL: Set = Set {
    track: line::VERTICAL,
    thumb: block::FULL,
    begin: "↑",
    end: "↓",
};

pub const HORIZONTAL: Set = Set {
    track: line::HORIZONTAL,
    thumb: block::FULL,
    begin: "←",
    end: "→",
};
