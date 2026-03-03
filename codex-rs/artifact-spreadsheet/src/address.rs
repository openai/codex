use serde::Deserialize;
use serde::Serialize;

use crate::SpreadsheetArtifactError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CellAddress {
    pub column: u32,
    pub row: u32,
}

impl CellAddress {
    pub fn parse(address: &str) -> Result<Self, SpreadsheetArtifactError> {
        let trimmed = address.trim();
        if trimmed.is_empty() {
            return Err(SpreadsheetArtifactError::InvalidAddress {
                address: address.to_string(),
                message: "address is empty".to_string(),
            });
        }

        let mut split = 0usize;
        for (index, ch) in trimmed.char_indices() {
            if ch.is_ascii_alphabetic() {
                split = index + ch.len_utf8();
            } else {
                break;
            }
        }

        let (letters, digits) = trimmed.split_at(split);
        if letters.is_empty() || digits.is_empty() {
            return Err(SpreadsheetArtifactError::InvalidAddress {
                address: address.to_string(),
                message: "expected A1-style address".to_string(),
            });
        }

        if !letters.chars().all(|ch| ch.is_ascii_alphabetic())
            || !digits.chars().all(|ch| ch.is_ascii_digit())
        {
            return Err(SpreadsheetArtifactError::InvalidAddress {
                address: address.to_string(),
                message: "expected letters followed by digits".to_string(),
            });
        }

        let column = column_letters_to_index(letters)?;
        let row = digits
            .parse::<u32>()
            .map_err(|_| SpreadsheetArtifactError::InvalidAddress {
                address: address.to_string(),
                message: "row must be a positive integer".to_string(),
            })?;

        if row == 0 {
            return Err(SpreadsheetArtifactError::InvalidAddress {
                address: address.to_string(),
                message: "row must be positive".to_string(),
            });
        }

        Ok(Self { column, row })
    }

    pub fn to_a1(self) -> String {
        format!("{}{}", column_index_to_letters(self.column), self.row)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRange {
    pub start: CellAddress,
    pub end: CellAddress,
}

impl CellRange {
    pub fn parse(address: &str) -> Result<Self, SpreadsheetArtifactError> {
        let trimmed = address.trim();
        if trimmed.is_empty() {
            return Err(SpreadsheetArtifactError::InvalidAddress {
                address: address.to_string(),
                message: "range is empty".to_string(),
            });
        }

        let (start, end) = if let Some((left, right)) = trimmed.split_once(':') {
            (CellAddress::parse(left)?, CellAddress::parse(right)?)
        } else {
            let cell = CellAddress::parse(trimmed)?;
            (cell, cell)
        };

        let normalized = Self {
            start: CellAddress {
                column: start.column.min(end.column),
                row: start.row.min(end.row),
            },
            end: CellAddress {
                column: start.column.max(end.column),
                row: start.row.max(end.row),
            },
        };
        Ok(normalized)
    }

    pub fn from_start_end(start: CellAddress, end: CellAddress) -> Self {
        Self {
            start: CellAddress {
                column: start.column.min(end.column),
                row: start.row.min(end.row),
            },
            end: CellAddress {
                column: start.column.max(end.column),
                row: start.row.max(end.row),
            },
        }
    }

    pub fn to_a1(&self) -> String {
        if self.is_single_cell() {
            self.start.to_a1()
        } else {
            format!("{}:{}", self.start.to_a1(), self.end.to_a1())
        }
    }

    pub fn is_single_cell(&self) -> bool {
        self.start == self.end
    }

    pub fn is_single_row(&self) -> bool {
        self.start.row == self.end.row
    }

    pub fn is_single_column(&self) -> bool {
        self.start.column == self.end.column
    }

    pub fn width(&self) -> usize {
        (self.end.column - self.start.column + 1) as usize
    }

    pub fn height(&self) -> usize {
        (self.end.row - self.start.row + 1) as usize
    }

    pub fn contains(&self, address: CellAddress) -> bool {
        self.start.column <= address.column
            && address.column <= self.end.column
            && self.start.row <= address.row
            && address.row <= self.end.row
    }

    pub fn contains_range(&self, other: &CellRange) -> bool {
        self.contains(other.start) && self.contains(other.end)
    }

    pub fn intersects(&self, other: &CellRange) -> bool {
        !(self.end.column < other.start.column
            || other.end.column < self.start.column
            || self.end.row < other.start.row
            || other.end.row < self.start.row)
    }

    pub fn addresses(&self) -> impl Iterator<Item = CellAddress> {
        let range = self.clone();
        (range.start.row..=range.end.row).flat_map(move |row| {
            let range = range.clone();
            (range.start.column..=range.end.column).map(move |column| CellAddress { column, row })
        })
    }
}

pub fn column_letters_to_index(column: &str) -> Result<u32, SpreadsheetArtifactError> {
    let trimmed = column.trim();
    if trimmed.is_empty() {
        return Err(SpreadsheetArtifactError::InvalidAddress {
            address: column.to_string(),
            message: "column is empty".to_string(),
        });
    }

    let mut result = 0u32;
    for ch in trimmed.chars() {
        if !ch.is_ascii_alphabetic() {
            return Err(SpreadsheetArtifactError::InvalidAddress {
                address: column.to_string(),
                message: "column must contain only letters".to_string(),
            });
        }
        result = result
            .checked_mul(26)
            .and_then(|value| value.checked_add((ch.to_ascii_uppercase() as u8 - b'A' + 1) as u32))
            .ok_or_else(|| SpreadsheetArtifactError::InvalidAddress {
                address: column.to_string(),
                message: "column is too large".to_string(),
            })?;
    }
    Ok(result)
}

pub fn column_index_to_letters(mut index: u32) -> String {
    if index == 0 {
        return String::new();
    }

    let mut letters = Vec::new();
    while index > 0 {
        let remainder = (index - 1) % 26;
        letters.push((b'A' + remainder as u8) as char);
        index = (index - 1) / 26;
    }
    letters.iter().rev().collect()
}

pub fn parse_column_reference(reference: &str) -> Result<(u32, u32), SpreadsheetArtifactError> {
    let trimmed = reference.trim();
    if let Some((left, right)) = trimmed.split_once(':') {
        let start = column_letters_to_index(left)?;
        let end = column_letters_to_index(right)?;
        Ok((start.min(end), start.max(end)))
    } else {
        let column = column_letters_to_index(trimmed)?;
        Ok((column, column))
    }
}

pub fn is_valid_cell_reference(address: &str) -> bool {
    CellAddress::parse(address).is_ok()
}

pub fn is_valid_range_reference(address: &str) -> bool {
    CellRange::parse(address).is_ok()
}

pub fn is_valid_row_reference(address: &str) -> bool {
    CellRange::parse(address)
        .map(|range| range.is_single_row())
        .unwrap_or(false)
}

pub fn is_valid_column_reference(address: &str) -> bool {
    parse_column_reference(address).is_ok()
}
