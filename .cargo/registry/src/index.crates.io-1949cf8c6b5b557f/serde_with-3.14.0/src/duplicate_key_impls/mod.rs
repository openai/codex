mod error_on_duplicate;
mod first_value_wins;
mod last_value_wins;

pub use self::{
    error_on_duplicate::{PreventDuplicateInsertsMap, PreventDuplicateInsertsSet},
    first_value_wins::DuplicateInsertsFirstWinsMap,
    last_value_wins::DuplicateInsertsLastWinsSet,
};
