use strum::{Display, EnumString};

/// This option allows the user to configure the "highlight symbol" column width spacing
#[derive(Debug, Display, EnumString, PartialEq, Eq, Clone, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HighlightSpacing {
    /// Always add spacing for the selection symbol column
    ///
    /// With this variant, the column for the selection symbol will always be allocated, and so the
    /// table will never change size, regardless of if a row is selected or not
    Always,

    /// Only add spacing for the selection symbol column if a row is selected
    ///
    /// With this variant, the column for the selection symbol will only be allocated if there is a
    /// selection, causing the table to shift if selected / unselected
    #[default]
    WhenSelected,

    /// Never add spacing to the selection symbol column, regardless of whether something is
    /// selected or not
    ///
    /// This means that the highlight symbol will never be drawn
    Never,
}

impl HighlightSpacing {
    /// Determine if a selection column should be displayed
    ///
    /// `has_selection`: true if a row is selected in the table
    ///
    /// Returns true if a selection column should be displayed
    pub(crate) const fn should_add(&self, has_selection: bool) -> bool {
        match self {
            Self::Always => true,
            Self::WhenSelected => has_selection,
            Self::Never => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn to_string() {
        assert_eq!(HighlightSpacing::Always.to_string(), "Always".to_string());
        assert_eq!(
            HighlightSpacing::WhenSelected.to_string(),
            "WhenSelected".to_string()
        );
        assert_eq!(HighlightSpacing::Never.to_string(), "Never".to_string());
    }

    #[test]
    fn from_str() {
        assert_eq!(
            "Always".parse::<HighlightSpacing>(),
            Ok(HighlightSpacing::Always)
        );
        assert_eq!(
            "WhenSelected".parse::<HighlightSpacing>(),
            Ok(HighlightSpacing::WhenSelected)
        );
        assert_eq!(
            "Never".parse::<HighlightSpacing>(),
            Ok(HighlightSpacing::Never)
        );
        assert_eq!(
            "".parse::<HighlightSpacing>(),
            Err(strum::ParseError::VariantNotFound)
        );
    }
}
