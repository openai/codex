use itertools::Itertools;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Clear, Padding, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState,
    StatefulWidget, Table, TableState, Widget, Wrap,
};

use crate::{RgbSwatch, THEME};

#[derive(Debug, Default, Clone, Copy)]
struct Ingredient {
    quantity: &'static str,
    name: &'static str,
}

impl Ingredient {
    #[expect(clippy::cast_possible_truncation)]
    fn height(&self) -> u16 {
        self.name.lines().count() as u16
    }
}

impl From<Ingredient> for Row<'_> {
    fn from(i: Ingredient) -> Self {
        Row::new(vec![i.quantity, i.name]).height(i.height())
    }
}

// https://www.realsimple.com/food-recipes/browse-all-recipes/ratatouille
const RECIPE: &[(&str, &str)] = &[
    (
        "Step 1: ",
        "Over medium-low heat, add the oil to a large skillet with the onion, garlic, and bay \
        leaf, stirring occasionally, until the onion has softened.",
    ),
    (
        "Step 2: ",
        "Add the eggplant and cook, stirring occasionally, for 8 minutes or until the eggplant \
        has softened. Stir in the zucchini, red bell pepper, tomatoes, and salt, and cook over \
        medium heat, stirring occasionally, for 5 to 7 minutes or until the vegetables are \
        tender. Stir in the basil and few grinds of pepper to taste.",
    ),
];

const INGREDIENTS: &[Ingredient] = &[
    Ingredient {
        quantity: "4 tbsp",
        name: "olive oil",
    },
    Ingredient {
        quantity: "1",
        name: "onion thinly sliced",
    },
    Ingredient {
        quantity: "4",
        name: "cloves garlic\npeeled and sliced",
    },
    Ingredient {
        quantity: "1",
        name: "small bay leaf",
    },
    Ingredient {
        quantity: "1",
        name: "small eggplant cut\ninto 1/2 inch cubes",
    },
    Ingredient {
        quantity: "1",
        name: "small zucchini halved\nlengthwise and cut\ninto thin slices",
    },
    Ingredient {
        quantity: "1",
        name: "red bell pepper cut\ninto slivers",
    },
    Ingredient {
        quantity: "4",
        name: "plum tomatoes\ncoarsely chopped",
    },
    Ingredient {
        quantity: "1 tsp",
        name: "kosher salt",
    },
    Ingredient {
        quantity: "1/4 cup",
        name: "shredded fresh basil\nleaves",
    },
    Ingredient {
        quantity: "",
        name: "freshly ground black\npepper",
    },
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RecipeTab {
    row_index: usize,
}

impl RecipeTab {
    /// Select the previous item in the ingredients list (with wrap around)
    pub fn prev(&mut self) {
        self.row_index = self.row_index.saturating_add(INGREDIENTS.len() - 1) % INGREDIENTS.len();
    }

    /// Select the next item in the ingredients list (with wrap around)
    pub fn next(&mut self) {
        self.row_index = self.row_index.saturating_add(1) % INGREDIENTS.len();
    }
}

impl Widget for RecipeTab {
    fn render(self, area: Rect, buf: &mut Buffer) {
        RgbSwatch.render(area, buf);
        let area = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });
        Clear.render(area, buf);
        Block::new()
            .title("Ratatouille Recipe".bold().white())
            .title_alignment(Alignment::Center)
            .style(THEME.content)
            .padding(Padding::new(1, 1, 2, 1))
            .render(area, buf);

        let scrollbar_area = Rect {
            y: area.y + 2,
            height: area.height - 3,
            ..area
        };
        render_scrollbar(self.row_index, scrollbar_area, buf);

        let area = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let layout = Layout::horizontal([Constraint::Length(44), Constraint::Min(0)]);
        let [recipe, ingredients] = area.layout(&layout);

        render_recipe(recipe, buf);
        render_ingredients(self.row_index, ingredients, buf);
    }
}

fn render_recipe(area: Rect, buf: &mut Buffer) {
    let lines = RECIPE
        .iter()
        .map(|(step, text)| Line::from(vec![step.white().bold(), text.gray()]))
        .collect_vec();
    Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .block(Block::new().padding(Padding::new(0, 1, 0, 0)))
        .render(area, buf);
}

fn render_ingredients(selected_row: usize, area: Rect, buf: &mut Buffer) {
    let mut state = TableState::default().with_selected(Some(selected_row));
    let rows = INGREDIENTS.iter().copied();
    let theme = THEME.recipe;
    StatefulWidget::render(
        Table::new(rows, [Constraint::Length(7), Constraint::Length(30)])
            .block(Block::new().style(theme.ingredients))
            .header(Row::new(vec!["Qty", "Ingredient"]).style(theme.ingredients_header))
            .row_highlight_style(Style::new().light_yellow()),
        area,
        buf,
        &mut state,
    );
}

fn render_scrollbar(position: usize, area: Rect, buf: &mut Buffer) {
    let mut state = ScrollbarState::default()
        .content_length(INGREDIENTS.len())
        .viewport_content_length(6)
        .position(position);
    Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_symbol(None)
        .thumb_symbol("▐")
        .render(area, buf, &mut state);
}
