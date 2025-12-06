use crate::symbols::{block, line};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Set<'a> {
    pub top_left: &'a str,
    pub top_right: &'a str,
    pub bottom_left: &'a str,
    pub bottom_right: &'a str,
    pub vertical_left: &'a str,
    pub vertical_right: &'a str,
    pub horizontal_top: &'a str,
    pub horizontal_bottom: &'a str,
}

impl Default for Set<'_> {
    fn default() -> Self {
        PLAIN
    }
}

// Helper function to convert a line set to a border set
const fn from_line_set(line_set: line::Set<'_>) -> Set<'_> {
    Set {
        top_left: line_set.top_left,
        top_right: line_set.top_right,
        bottom_left: line_set.bottom_left,
        bottom_right: line_set.bottom_right,
        vertical_left: line_set.vertical,
        vertical_right: line_set.vertical,
        horizontal_top: line_set.horizontal,
        horizontal_bottom: line_set.horizontal,
    }
}

/// Border Set with a single line width
///
/// ```text
/// ┌─────┐
/// │xxxxx│
/// │xxxxx│
/// └─────┘
/// ```
pub const PLAIN: Set = Set {
    top_left: line::NORMAL.top_left,
    top_right: line::NORMAL.top_right,
    bottom_left: line::NORMAL.bottom_left,
    bottom_right: line::NORMAL.bottom_right,
    vertical_left: line::NORMAL.vertical,
    vertical_right: line::NORMAL.vertical,
    horizontal_top: line::NORMAL.horizontal,
    horizontal_bottom: line::NORMAL.horizontal,
};

/// Border Set with a single line width and rounded corners
///
/// ```text
/// ╭─────╮
/// │xxxxx│
/// │xxxxx│
/// ╰─────╯
/// ```
pub const ROUNDED: Set = Set {
    top_left: line::ROUNDED.top_left,
    top_right: line::ROUNDED.top_right,
    bottom_left: line::ROUNDED.bottom_left,
    bottom_right: line::ROUNDED.bottom_right,
    vertical_left: line::ROUNDED.vertical,
    vertical_right: line::ROUNDED.vertical,
    horizontal_top: line::ROUNDED.horizontal,
    horizontal_bottom: line::ROUNDED.horizontal,
};

/// Border Set with a double line width
///
/// ```text
/// ╔═════╗
/// ║xxxxx║
/// ║xxxxx║
/// ╚═════╝
/// ```
pub const DOUBLE: Set = Set {
    top_left: line::DOUBLE.top_left,
    top_right: line::DOUBLE.top_right,
    bottom_left: line::DOUBLE.bottom_left,
    bottom_right: line::DOUBLE.bottom_right,
    vertical_left: line::DOUBLE.vertical,
    vertical_right: line::DOUBLE.vertical,
    horizontal_top: line::DOUBLE.horizontal,
    horizontal_bottom: line::DOUBLE.horizontal,
};

/// Border Set with a thick line width
///
/// ```text
/// ┏━━━━━┓
/// ┃xxxxx┃
/// ┃xxxxx┃
/// ┗━━━━━┛
/// ```
pub const THICK: Set = Set {
    top_left: line::THICK.top_left,
    top_right: line::THICK.top_right,
    bottom_left: line::THICK.bottom_left,
    bottom_right: line::THICK.bottom_right,
    vertical_left: line::THICK.vertical,
    vertical_right: line::THICK.vertical,
    horizontal_top: line::THICK.horizontal,
    horizontal_bottom: line::THICK.horizontal,
};

/// Border Set with light double-dashed border lines
///
/// ```text
/// ┌╌╌╌╌╌┐
/// ╎xxxxx╎
/// ╎xxxxx╎
/// └╌╌╌╌╌┘
/// ```
pub const LIGHT_DOUBLE_DASHED: Set = from_line_set(line::LIGHT_DOUBLE_DASHED);

/// Border Set with thick double-dashed border lines
///
/// ```text
/// ┏╍╍╍╍╍┓
/// ╏xxxxx╏
/// ╏xxxxx╏
/// ┗╍╍╍╍╍┛
/// ```
pub const HEAVY_DOUBLE_DASHED: Set = from_line_set(line::HEAVY_DOUBLE_DASHED);

/// Border Set with light triple-dashed border lines
///
/// ```text
/// ┌┄┄┄┄┄┐
/// ┆xxxxx┆
/// ┆xxxxx┆
/// └┄┄┄┄┄┘
/// ```
pub const LIGHT_TRIPLE_DASHED: Set = from_line_set(line::LIGHT_TRIPLE_DASHED);

/// Border Set with thick triple-dashed border lines
///
/// ```text
/// ┏┅┅┅┅┅┓
/// ┇xxxxx┇
/// ┇xxxxx┇
/// ┗┅┅┅┅┅┛
/// ```
pub const HEAVY_TRIPLE_DASHED: Set = from_line_set(line::HEAVY_TRIPLE_DASHED);

/// Border Set with light quadruple-dashed border lines
///
/// ```text
/// ┌┈┈┈┈┈┐
/// ┊xxxxx┊
/// ┊xxxxx┊
/// └┈┈┈┈┈┘
/// ```
pub const LIGHT_QUADRUPLE_DASHED: Set = from_line_set(line::LIGHT_QUADRUPLE_DASHED);

/// Border Set with thick quadruple-dashed border lines
///
/// ```text
/// ┏┉┉┉┉┉┓
/// ┋xxxxx┋
/// ┋xxxxx┋
/// ┗┉┉┉┉┉┛
/// ```
pub const HEAVY_QUADRUPLE_DASHED: Set = from_line_set(line::HEAVY_QUADRUPLE_DASHED);

pub const QUADRANT_TOP_LEFT: &str = "▘";
pub const QUADRANT_TOP_RIGHT: &str = "▝";
pub const QUADRANT_BOTTOM_LEFT: &str = "▖";
pub const QUADRANT_BOTTOM_RIGHT: &str = "▗";
pub const QUADRANT_TOP_HALF: &str = "▀";
pub const QUADRANT_BOTTOM_HALF: &str = "▄";
pub const QUADRANT_LEFT_HALF: &str = "▌";
pub const QUADRANT_RIGHT_HALF: &str = "▐";
pub const QUADRANT_TOP_LEFT_BOTTOM_LEFT_BOTTOM_RIGHT: &str = "▙";
pub const QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_LEFT: &str = "▛";
pub const QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_RIGHT: &str = "▜";
pub const QUADRANT_TOP_RIGHT_BOTTOM_LEFT_BOTTOM_RIGHT: &str = "▟";
pub const QUADRANT_TOP_LEFT_BOTTOM_RIGHT: &str = "▚";
pub const QUADRANT_TOP_RIGHT_BOTTOM_LEFT: &str = "▞";
pub const QUADRANT_BLOCK: &str = "█";

/// Quadrant used for setting a border outside a block by one half cell "pixel".
///
/// ```text
/// ▛▀▀▀▀▀▜
/// ▌xxxxx▐
/// ▌xxxxx▐
/// ▙▄▄▄▄▄▟
/// ```
pub const QUADRANT_OUTSIDE: Set = Set {
    top_left: QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_LEFT,
    top_right: QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_RIGHT,
    bottom_left: QUADRANT_TOP_LEFT_BOTTOM_LEFT_BOTTOM_RIGHT,
    bottom_right: QUADRANT_TOP_RIGHT_BOTTOM_LEFT_BOTTOM_RIGHT,
    vertical_left: QUADRANT_LEFT_HALF,
    vertical_right: QUADRANT_RIGHT_HALF,
    horizontal_top: QUADRANT_TOP_HALF,
    horizontal_bottom: QUADRANT_BOTTOM_HALF,
};

/// Quadrant used for setting a border inside a block by one half cell "pixel".
///
/// ```text
/// ▗▄▄▄▄▄▖
/// ▐xxxxx▌
/// ▐xxxxx▌
/// ▝▀▀▀▀▀▘
/// ```
pub const QUADRANT_INSIDE: Set = Set {
    top_right: QUADRANT_BOTTOM_LEFT,
    top_left: QUADRANT_BOTTOM_RIGHT,
    bottom_right: QUADRANT_TOP_LEFT,
    bottom_left: QUADRANT_TOP_RIGHT,
    vertical_left: QUADRANT_RIGHT_HALF,
    vertical_right: QUADRANT_LEFT_HALF,
    horizontal_top: QUADRANT_BOTTOM_HALF,
    horizontal_bottom: QUADRANT_TOP_HALF,
};

pub const ONE_EIGHTH_TOP_EIGHT: &str = "▔";
pub const ONE_EIGHTH_BOTTOM_EIGHT: &str = "▁";
pub const ONE_EIGHTH_LEFT_EIGHT: &str = "▏";
pub const ONE_EIGHTH_RIGHT_EIGHT: &str = "▕";

/// Wide border set based on McGugan box technique
///
/// ```text
/// ▁▁▁▁▁▁▁
/// ▏xxxxx▕
/// ▏xxxxx▕
/// ▔▔▔▔▔▔▔
/// ```
#[expect(clippy::doc_markdown)]
pub const ONE_EIGHTH_WIDE: Set = Set {
    top_right: ONE_EIGHTH_BOTTOM_EIGHT,
    top_left: ONE_EIGHTH_BOTTOM_EIGHT,
    bottom_right: ONE_EIGHTH_TOP_EIGHT,
    bottom_left: ONE_EIGHTH_TOP_EIGHT,
    vertical_left: ONE_EIGHTH_LEFT_EIGHT,
    vertical_right: ONE_EIGHTH_RIGHT_EIGHT,
    horizontal_top: ONE_EIGHTH_BOTTOM_EIGHT,
    horizontal_bottom: ONE_EIGHTH_TOP_EIGHT,
};

/// Tall border set based on McGugan box technique
///
/// ```text
/// ▕▔▔▏
/// ▕xx▏
/// ▕xx▏
/// ▕▁▁▏
/// ```
#[expect(clippy::doc_markdown)]
pub const ONE_EIGHTH_TALL: Set = Set {
    top_right: ONE_EIGHTH_LEFT_EIGHT,
    top_left: ONE_EIGHTH_RIGHT_EIGHT,
    bottom_right: ONE_EIGHTH_LEFT_EIGHT,
    bottom_left: ONE_EIGHTH_RIGHT_EIGHT,
    vertical_left: ONE_EIGHTH_RIGHT_EIGHT,
    vertical_right: ONE_EIGHTH_LEFT_EIGHT,
    horizontal_top: ONE_EIGHTH_TOP_EIGHT,
    horizontal_bottom: ONE_EIGHTH_BOTTOM_EIGHT,
};

/// Wide proportional (visually equal width and height) border with using set of quadrants.
///
/// The border is created by using half blocks for top and bottom, and full
/// blocks for right and left sides to make horizontal and vertical borders seem equal.
///
/// ```text
/// ▄▄▄▄
/// █xx█
/// █xx█
/// ▀▀▀▀
/// ```
pub const PROPORTIONAL_WIDE: Set = Set {
    top_right: QUADRANT_BOTTOM_HALF,
    top_left: QUADRANT_BOTTOM_HALF,
    bottom_right: QUADRANT_TOP_HALF,
    bottom_left: QUADRANT_TOP_HALF,
    vertical_left: QUADRANT_BLOCK,
    vertical_right: QUADRANT_BLOCK,
    horizontal_top: QUADRANT_BOTTOM_HALF,
    horizontal_bottom: QUADRANT_TOP_HALF,
};

/// Tall proportional (visually equal width and height) border with using set of quadrants.
///
/// The border is created by using full blocks for all sides, except for the top and bottom,
/// which use half blocks to make horizontal and vertical borders seem equal.
///
/// ```text
/// ▕█▀▀█
/// ▕█xx█
/// ▕█xx█
/// ▕█▄▄█
/// ```
pub const PROPORTIONAL_TALL: Set = Set {
    top_right: QUADRANT_BLOCK,
    top_left: QUADRANT_BLOCK,
    bottom_right: QUADRANT_BLOCK,
    bottom_left: QUADRANT_BLOCK,
    vertical_left: QUADRANT_BLOCK,
    vertical_right: QUADRANT_BLOCK,
    horizontal_top: QUADRANT_TOP_HALF,
    horizontal_bottom: QUADRANT_BOTTOM_HALF,
};

/// Solid border set
///
/// The border is created by using full blocks for all sides.
///
/// ```text
/// ████
/// █xx█
/// █xx█
/// ████
/// ```
pub const FULL: Set = Set {
    top_left: block::FULL,
    top_right: block::FULL,
    bottom_left: block::FULL,
    bottom_right: block::FULL,
    vertical_left: block::FULL,
    vertical_right: block::FULL,
    horizontal_top: block::FULL,
    horizontal_bottom: block::FULL,
};

/// Empty border set
///
/// The border is created by using empty strings for all sides.
///
/// This is useful for ensuring that the border style is applied to a border on a block with a title
/// without actually drawing a border.
///
/// ░ Example
///
/// `░` represents the content in the area not covered by the border to make it easier to see the
/// blank symbols.
///
/// ```text
/// ░░░░░░░░
/// ░░    ░░
/// ░░ ░░ ░░
/// ░░ ░░ ░░
/// ░░    ░░
/// ░░░░░░░░
/// ```
pub const EMPTY: Set = Set {
    top_left: " ",
    top_right: " ",
    bottom_left: " ",
    bottom_right: " ",
    vertical_left: " ",
    vertical_right: " ",
    horizontal_top: " ",
    horizontal_bottom: " ",
};

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;

    use indoc::{formatdoc, indoc};

    use super::*;

    #[test]
    fn default() {
        assert_eq!(Set::default(), PLAIN);
    }

    /// A helper function to render a border set to a string.
    ///
    /// '░' (U+2591 Light Shade) is used as a placeholder for empty space to make it easier to see
    /// the size of the border symbols.
    fn render(set: Set) -> String {
        formatdoc!(
            "░░░░░░
             ░{}{}{}{}░
             ░{}░░{}░
             ░{}░░{}░
             ░{}{}{}{}░
             ░░░░░░",
            set.top_left,
            set.horizontal_top,
            set.horizontal_top,
            set.top_right,
            set.vertical_left,
            set.vertical_right,
            set.vertical_left,
            set.vertical_right,
            set.bottom_left,
            set.horizontal_bottom,
            set.horizontal_bottom,
            set.bottom_right
        )
    }

    #[test]
    fn border_set_from_line_set() {
        let custom_line_set = line::Set {
            top_left: "a",
            top_right: "b",
            bottom_left: "c",
            bottom_right: "d",
            vertical: "e",
            horizontal: "f",
            vertical_left: "g",
            vertical_right: "h",
            horizontal_down: "i",
            horizontal_up: "j",
            cross: "k",
        };

        let border_set = from_line_set(custom_line_set);

        assert_eq!(
            border_set,
            Set {
                top_left: "a",
                top_right: "b",
                bottom_left: "c",
                bottom_right: "d",
                vertical_left: "e",
                vertical_right: "e",
                horizontal_bottom: "f",
                horizontal_top: "f",
            }
        );
    }

    #[test]
    fn plain() {
        assert_eq!(
            render(PLAIN),
            indoc!(
                "░░░░░░
                 ░┌──┐░
                 ░│░░│░
                 ░│░░│░
                 ░└──┘░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn rounded() {
        assert_eq!(
            render(ROUNDED),
            indoc!(
                "░░░░░░
                 ░╭──╮░
                 ░│░░│░
                 ░│░░│░
                 ░╰──╯░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn double() {
        assert_eq!(
            render(DOUBLE),
            indoc!(
                "░░░░░░
                 ░╔══╗░
                 ░║░░║░
                 ░║░░║░
                 ░╚══╝░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn thick() {
        assert_eq!(
            render(THICK),
            indoc!(
                "░░░░░░
                 ░┏━━┓░
                 ░┃░░┃░
                 ░┃░░┃░
                 ░┗━━┛░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn light_double_dashed() {
        assert_eq!(
            render(LIGHT_DOUBLE_DASHED),
            indoc!(
                "░░░░░░
                 ░┌╌╌┐░
                 ░╎░░╎░
                 ░╎░░╎░
                 ░└╌╌┘░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn heavy_double_dashed() {
        assert_eq!(
            render(HEAVY_DOUBLE_DASHED),
            indoc!(
                "░░░░░░
                 ░┏╍╍┓░
                 ░╏░░╏░
                 ░╏░░╏░
                 ░┗╍╍┛░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn light_triple_dashed() {
        assert_eq!(
            render(LIGHT_TRIPLE_DASHED),
            indoc!(
                "░░░░░░
                 ░┌┄┄┐░
                 ░┆░░┆░
                 ░┆░░┆░
                 ░└┄┄┘░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn heavy_triple_dashed() {
        assert_eq!(
            render(HEAVY_TRIPLE_DASHED),
            indoc!(
                "░░░░░░
                 ░┏┅┅┓░
                 ░┇░░┇░
                 ░┇░░┇░
                 ░┗┅┅┛░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn light_quadruple_dashed() {
        assert_eq!(
            render(LIGHT_QUADRUPLE_DASHED),
            indoc!(
                "░░░░░░
                 ░┌┈┈┐░
                 ░┊░░┊░
                 ░┊░░┊░
                 ░└┈┈┘░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn heavy_quadruple_dashed() {
        assert_eq!(
            render(HEAVY_QUADRUPLE_DASHED),
            indoc!(
                "░░░░░░
                 ░┏┉┉┓░
                 ░┋░░┋░
                 ░┋░░┋░
                 ░┗┉┉┛░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn quadrant_outside() {
        assert_eq!(
            render(QUADRANT_OUTSIDE),
            indoc!(
                "░░░░░░
                 ░▛▀▀▜░
                 ░▌░░▐░
                 ░▌░░▐░
                 ░▙▄▄▟░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn quadrant_inside() {
        assert_eq!(
            render(QUADRANT_INSIDE),
            indoc!(
                "░░░░░░
                 ░▗▄▄▖░
                 ░▐░░▌░
                 ░▐░░▌░
                 ░▝▀▀▘░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn one_eighth_wide() {
        assert_eq!(
            render(ONE_EIGHTH_WIDE),
            indoc!(
                "░░░░░░
                 ░▁▁▁▁░
                 ░▏░░▕░
                 ░▏░░▕░
                 ░▔▔▔▔░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn one_eighth_tall() {
        assert_eq!(
            render(ONE_EIGHTH_TALL),
            indoc!(
                "░░░░░░
                 ░▕▔▔▏░
                 ░▕░░▏░
                 ░▕░░▏░
                 ░▕▁▁▏░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn proportional_wide() {
        assert_eq!(
            render(PROPORTIONAL_WIDE),
            indoc!(
                "░░░░░░
                 ░▄▄▄▄░
                 ░█░░█░
                 ░█░░█░
                 ░▀▀▀▀░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn proportional_tall() {
        assert_eq!(
            render(PROPORTIONAL_TALL),
            indoc!(
                "░░░░░░
                 ░█▀▀█░
                 ░█░░█░
                 ░█░░█░
                 ░█▄▄█░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn full() {
        assert_eq!(
            render(FULL),
            indoc!(
                "░░░░░░
                 ░████░
                 ░█░░█░
                 ░█░░█░
                 ░████░
                 ░░░░░░"
            )
        );
    }

    #[test]
    fn empty() {
        assert_eq!(
            render(EMPTY),
            indoc!(
                "░░░░░░
                 ░    ░
                 ░ ░░ ░
                 ░ ░░ ░
                 ░    ░
                 ░░░░░░"
            )
        );
    }
}
