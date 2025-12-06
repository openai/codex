use alloc::vec::Vec;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Rect};
use ratatui_core::style::Style;
use ratatui_core::text::Line;
use ratatui_core::widgets::Widget;

use crate::barchart::Bar;

/// A group of bars to be shown by the Barchart.
///
/// # Examples
///
/// ```
/// use ratatui::widgets::{Bar, BarGroup};
///
/// let group = BarGroup::new([Bar::with_label("Red", 20), Bar::with_label("Blue", 15)]);
/// ```
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct BarGroup<'a> {
    /// label of the group. It will be printed centered under this group of bars
    pub(super) label: Option<Line<'a>>,
    /// list of bars to be shown
    pub(super) bars: Vec<Bar<'a>>,
}

impl<'a> BarGroup<'a> {
    /// Creates a new `BarGroup` with the given bars.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::style::{Style, Stylize};
    /// use ratatui::widgets::{Bar, BarGroup};
    ///
    /// let group = BarGroup::new(vec![Bar::with_label("A", 10), Bar::with_label("B", 20)]);
    /// ```
    pub fn new<T: Into<Vec<Bar<'a>>>>(bars: T) -> Self {
        Self {
            bars: bars.into(),
            ..Self::default()
        }
    }

    /// Creates a new `BarGroup` with the given bars and label.
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::style::{Style, Stylize};
    /// use ratatui::widgets::{Bar, BarGroup};
    ///
    /// let group = BarGroup::with_label(
    ///     "Group1",
    ///     vec![Bar::with_label("A", 10), Bar::with_label("B", 20)],
    /// );
    /// ```
    pub fn with_label<T: Into<Line<'a>>, B: Into<Vec<Bar<'a>>>>(label: T, bars: B) -> Self {
        Self {
            label: Some(label.into()),
            bars: bars.into(),
        }
    }

    /// Set the group label
    ///
    /// `label` can be a [`&str`], [`String`] or anything that can be converted into [`Line`].
    ///
    /// # Examples
    ///
    /// From [`&str`] and [`String`].
    ///
    /// ```rust
    /// use ratatui::widgets::BarGroup;
    ///
    /// BarGroup::default().label("label");
    /// BarGroup::default().label(String::from("label"));
    /// ```
    ///
    /// From a [`Line`] with red foreground color:
    ///
    /// ```rust
    /// use ratatui::style::Stylize;
    /// use ratatui::text::Line;
    /// use ratatui::widgets::BarGroup;
    ///
    /// BarGroup::default().label(Line::from("Line").red());
    /// ```
    /// [`String`]: alloc::string::String
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn label<T: Into<Line<'a>>>(mut self, label: T) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the bars of the group to be shown
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn bars(mut self, bars: &[Bar<'a>]) -> Self {
        self.bars = bars.to_vec();
        self
    }

    /// The maximum bar value of this group
    pub(super) fn max(&self) -> Option<u64> {
        self.bars.iter().max_by_key(|v| v.value).map(|v| v.value)
    }

    pub(super) fn render_label(&self, buf: &mut Buffer, area: Rect, default_label_style: Style) {
        if let Some(label) = &self.label {
            // align the label. Necessary to do it this way as we don't want to set the style
            // of the whole area, just the label area
            let width = label.width() as u16;
            let area = match label.alignment {
                Some(Alignment::Center) => Rect {
                    x: area.x + (area.width.saturating_sub(width)) / 2,
                    width,
                    ..area
                },
                Some(Alignment::Right) => Rect {
                    x: area.x + area.width.saturating_sub(width),
                    width,
                    ..area
                },
                _ => Rect { width, ..area },
            };
            buf.set_style(area, default_label_style);
            label.render(area, buf);
        }
    }
}

impl<'a> From<&[(&'a str, u64)]> for BarGroup<'a> {
    fn from(value: &[(&'a str, u64)]) -> Self {
        Self {
            label: None,
            bars: value
                .iter()
                .map(|&(text, v)| Bar::with_label(text, v))
                .collect(),
        }
    }
}

impl<'a, const N: usize> From<&[(&'a str, u64); N]> for BarGroup<'a> {
    fn from(value: &[(&'a str, u64); N]) -> Self {
        let value: &[(&'a str, u64)] = value.as_ref();
        Self::from(value)
    }
}

impl<'a> From<&Vec<(&'a str, u64)>> for BarGroup<'a> {
    fn from(value: &Vec<(&'a str, u64)>) -> Self {
        let array: &[(&str, u64)] = value;
        Self::from(array)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bargroup_new() {
        let group = BarGroup::new([Bar::with_label("Label1", 1), Bar::with_label("Label2", 2)])
            .label(Line::from("Group1"));
        assert_eq!(group.label, Some(Line::from("Group1")));
        assert_eq!(group.bars.len(), 2);
    }
}
