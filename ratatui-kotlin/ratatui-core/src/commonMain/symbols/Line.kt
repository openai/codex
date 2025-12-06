pub const VERTICAL: &str = "│";
pub const DOUBLE_VERTICAL: &str = "║";
pub const THICK_VERTICAL: &str = "┃";
pub const LIGHT_DOUBLE_DASH_VERTICAL: &str = "╎";
pub const HEAVY_DOUBLE_DASH_VERTICAL: &str = "╏";
pub const LIGHT_TRIPLE_DASH_VERTICAL: &str = "┆";
pub const HEAVY_TRIPLE_DASH_VERTICAL: &str = "┇";
pub const LIGHT_QUADRUPLE_DASH_VERTICAL: &str = "┊";
pub const HEAVY_QUADRUPLE_DASH_VERTICAL: &str = "┋";

pub const HORIZONTAL: &str = "─";
pub const DOUBLE_HORIZONTAL: &str = "═";
pub const THICK_HORIZONTAL: &str = "━";
pub const LIGHT_DOUBLE_DASH_HORIZONTAL: &str = "╌";
pub const HEAVY_DOUBLE_DASH_HORIZONTAL: &str = "╍";
pub const LIGHT_TRIPLE_DASH_HORIZONTAL: &str = "┄";
pub const HEAVY_TRIPLE_DASH_HORIZONTAL: &str = "┅";
pub const LIGHT_QUADRUPLE_DASH_HORIZONTAL: &str = "┈";
pub const HEAVY_QUADRUPLE_DASH_HORIZONTAL: &str = "┉";

pub const TOP_RIGHT: &str = "┐";
pub const ROUNDED_TOP_RIGHT: &str = "╮";
pub const DOUBLE_TOP_RIGHT: &str = "╗";
pub const THICK_TOP_RIGHT: &str = "┓";

pub const TOP_LEFT: &str = "┌";
pub const ROUNDED_TOP_LEFT: &str = "╭";
pub const DOUBLE_TOP_LEFT: &str = "╔";
pub const THICK_TOP_LEFT: &str = "┏";

pub const BOTTOM_RIGHT: &str = "┘";
pub const ROUNDED_BOTTOM_RIGHT: &str = "╯";
pub const DOUBLE_BOTTOM_RIGHT: &str = "╝";
pub const THICK_BOTTOM_RIGHT: &str = "┛";

pub const BOTTOM_LEFT: &str = "└";
pub const ROUNDED_BOTTOM_LEFT: &str = "╰";
pub const DOUBLE_BOTTOM_LEFT: &str = "╚";
pub const THICK_BOTTOM_LEFT: &str = "┗";

pub const VERTICAL_LEFT: &str = "┤";
pub const DOUBLE_VERTICAL_LEFT: &str = "╣";
pub const THICK_VERTICAL_LEFT: &str = "┫";

pub const VERTICAL_RIGHT: &str = "├";
pub const DOUBLE_VERTICAL_RIGHT: &str = "╠";
pub const THICK_VERTICAL_RIGHT: &str = "┣";

pub const HORIZONTAL_DOWN: &str = "┬";
pub const DOUBLE_HORIZONTAL_DOWN: &str = "╦";
pub const THICK_HORIZONTAL_DOWN: &str = "┳";

pub const HORIZONTAL_UP: &str = "┴";
pub const DOUBLE_HORIZONTAL_UP: &str = "╩";
pub const THICK_HORIZONTAL_UP: &str = "┻";

pub const CROSS: &str = "┼";
pub const DOUBLE_CROSS: &str = "╬";
pub const THICK_CROSS: &str = "╋";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Set<'a> {
    pub vertical: &'a str,
    pub horizontal: &'a str,
    pub top_right: &'a str,
    pub top_left: &'a str,
    pub bottom_right: &'a str,
    pub bottom_left: &'a str,
    pub vertical_left: &'a str,
    pub vertical_right: &'a str,
    pub horizontal_down: &'a str,
    pub horizontal_up: &'a str,
    pub cross: &'a str,
}

impl Default for Set<'_> {
    fn default() -> Self {
        NORMAL
    }
}

pub const NORMAL: Set = Set {
    vertical: VERTICAL,
    horizontal: HORIZONTAL,
    top_right: TOP_RIGHT,
    top_left: TOP_LEFT,
    bottom_right: BOTTOM_RIGHT,
    bottom_left: BOTTOM_LEFT,
    vertical_left: VERTICAL_LEFT,
    vertical_right: VERTICAL_RIGHT,
    horizontal_down: HORIZONTAL_DOWN,
    horizontal_up: HORIZONTAL_UP,
    cross: CROSS,
};

pub const ROUNDED: Set = Set {
    top_right: ROUNDED_TOP_RIGHT,
    top_left: ROUNDED_TOP_LEFT,
    bottom_right: ROUNDED_BOTTOM_RIGHT,
    bottom_left: ROUNDED_BOTTOM_LEFT,
    ..NORMAL
};

pub const DOUBLE: Set = Set {
    vertical: DOUBLE_VERTICAL,
    horizontal: DOUBLE_HORIZONTAL,
    top_right: DOUBLE_TOP_RIGHT,
    top_left: DOUBLE_TOP_LEFT,
    bottom_right: DOUBLE_BOTTOM_RIGHT,
    bottom_left: DOUBLE_BOTTOM_LEFT,
    vertical_left: DOUBLE_VERTICAL_LEFT,
    vertical_right: DOUBLE_VERTICAL_RIGHT,
    horizontal_down: DOUBLE_HORIZONTAL_DOWN,
    horizontal_up: DOUBLE_HORIZONTAL_UP,
    cross: DOUBLE_CROSS,
};

pub const THICK: Set = Set {
    vertical: THICK_VERTICAL,
    horizontal: THICK_HORIZONTAL,
    top_right: THICK_TOP_RIGHT,
    top_left: THICK_TOP_LEFT,
    bottom_right: THICK_BOTTOM_RIGHT,
    bottom_left: THICK_BOTTOM_LEFT,
    vertical_left: THICK_VERTICAL_LEFT,
    vertical_right: THICK_VERTICAL_RIGHT,
    horizontal_down: THICK_HORIZONTAL_DOWN,
    horizontal_up: THICK_HORIZONTAL_UP,
    cross: THICK_CROSS,
};

pub const LIGHT_DOUBLE_DASHED: Set = Set {
    vertical: LIGHT_DOUBLE_DASH_VERTICAL,
    horizontal: LIGHT_DOUBLE_DASH_HORIZONTAL,
    ..NORMAL
};

pub const HEAVY_DOUBLE_DASHED: Set = Set {
    vertical: HEAVY_DOUBLE_DASH_VERTICAL,
    horizontal: HEAVY_DOUBLE_DASH_HORIZONTAL,
    ..THICK
};

pub const LIGHT_TRIPLE_DASHED: Set = Set {
    vertical: LIGHT_TRIPLE_DASH_VERTICAL,
    horizontal: LIGHT_TRIPLE_DASH_HORIZONTAL,
    ..NORMAL
};

pub const HEAVY_TRIPLE_DASHED: Set = Set {
    vertical: HEAVY_TRIPLE_DASH_VERTICAL,
    horizontal: HEAVY_TRIPLE_DASH_HORIZONTAL,
    ..THICK
};

pub const LIGHT_QUADRUPLE_DASHED: Set = Set {
    vertical: LIGHT_QUADRUPLE_DASH_VERTICAL,
    horizontal: LIGHT_QUADRUPLE_DASH_HORIZONTAL,
    ..NORMAL
};

pub const HEAVY_QUADRUPLE_DASHED: Set = Set {
    vertical: HEAVY_QUADRUPLE_DASH_VERTICAL,
    horizontal: HEAVY_QUADRUPLE_DASH_HORIZONTAL,
    ..THICK
};

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;

    use indoc::{formatdoc, indoc};

    use super::*;

    #[test]
    fn default() {
        assert_eq!(Set::default(), NORMAL);
    }

    /// A helper function to render a set of symbols.
    fn render(set: Set) -> String {
        formatdoc!(
            "{}{}{}{}
             {}{}{}{}
             {}{}{}{}
             {}{}{}{}",
            set.top_left,
            set.horizontal,
            set.horizontal_down,
            set.top_right,
            set.vertical,
            " ",
            set.vertical,
            set.vertical,
            set.vertical_right,
            set.horizontal,
            set.cross,
            set.vertical_left,
            set.bottom_left,
            set.horizontal,
            set.horizontal_up,
            set.bottom_right
        )
    }

    #[test]
    fn normal() {
        assert_eq!(
            render(NORMAL),
            indoc!(
                "┌─┬┐
                 │ ││
                 ├─┼┤
                 └─┴┘"
            )
        );
    }

    #[test]
    fn rounded() {
        assert_eq!(
            render(ROUNDED),
            indoc!(
                "╭─┬╮
                 │ ││
                 ├─┼┤
                 ╰─┴╯"
            )
        );
    }

    #[test]
    fn double() {
        assert_eq!(
            render(DOUBLE),
            indoc!(
                "╔═╦╗
                 ║ ║║
                 ╠═╬╣
                 ╚═╩╝"
            )
        );
    }

    #[test]
    fn thick() {
        assert_eq!(
            render(THICK),
            indoc!(
                "┏━┳┓
                 ┃ ┃┃
                 ┣━╋┫
                 ┗━┻┛"
            )
        );
    }
}
