//! Material design color palettes.
//!
//! Represents the colors from the 2014 [Material design color palettes][palettes] by Google.
//!
//! [palettes]: https://m2.material.io/design/color/the-color-system.html#tools-for-picking-colors
//!
//! There are 16 palettes with accent colors, and 3 palettes without accent colors. Each palette
//! has 10 colors, with variants from 50 to 900. The accent palettes also have 4 accent colors
//! with variants from 100 to 700. Black and White are also included for completeness and to avoid
//! being affected by any terminal theme that might be in use.
//!
//! This module exists to provide a convenient way to use the colors from the
//! [`matdesign-color` crate] in your application.
//!
//! <style>
//! .color { display: flex; align-items: center; }
//! .color > div { width: 2rem; height: 2rem; }
//! .color > div.name { width: 150px; !important; }
//! </style>
//! <div style="overflow-x: auto">
//! <div style="display: flex; flex-direction:column; text-align: left">
//! <div class="color" style="font-size:0.8em">
//!     <div class="name"></div>
//!     <div>C50</div>
//!     <div>C100</div>
//!     <div>C200</div>
//!     <div>C300</div>
//!     <div>C400</div>
//!     <div>C500</div>
//!     <div>C600</div>
//!     <div>C700</div>
//!     <div>C800</div>
//!     <div>C900</div>
//!     <div>A100</div>
//!     <div>A200</div>
//!     <div>A400</div>
//!     <div>A700</div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`RED`]</div>
//!     <div style="background-color: #FFEBEE"></div>
//!     <div style="background-color: #FFCDD2"></div>
//!     <div style="background-color: #EF9A9A"></div>
//!     <div style="background-color: #E57373"></div>
//!     <div style="background-color: #EF5350"></div>
//!     <div style="background-color: #F44336"></div>
//!     <div style="background-color: #E53935"></div>
//!     <div style="background-color: #D32F2F"></div>
//!     <div style="background-color: #C62828"></div>
//!     <div style="background-color: #B71C1C"></div>
//!     <div style="background-color: #FF8A80"></div>
//!     <div style="background-color: #FF5252"></div>
//!     <div style="background-color: #FF1744"></div>
//!     <div style="background-color: #D50000"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`PINK`]</div>
//!     <div style="background-color: #FCE4EC"></div>
//!     <div style="background-color: #F8BBD0"></div>
//!     <div style="background-color: #F48FB1"></div>
//!     <div style="background-color: #F06292"></div>
//!     <div style="background-color: #EC407A"></div>
//!     <div style="background-color: #E91E63"></div>
//!     <div style="background-color: #D81B60"></div>
//!     <div style="background-color: #C2185B"></div>
//!     <div style="background-color: #AD1457"></div>
//!     <div style="background-color: #880E4F"></div>
//!     <div style="background-color: #FF80AB"></div>
//!     <div style="background-color: #FF4081"></div>
//!     <div style="background-color: #F50057"></div>
//!     <div style="background-color: #C51162"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`PURPLE`]</div>
//!     <div style="background-color: #F3E5F5"></div>
//!     <div style="background-color: #E1BEE7"></div>
//!     <div style="background-color: #CE93D8"></div>
//!     <div style="background-color: #BA68C8"></div>
//!     <div style="background-color: #AB47BC"></div>
//!     <div style="background-color: #9C27B0"></div>
//!     <div style="background-color: #8E24AA"></div>
//!     <div style="background-color: #7B1FA2"></div>
//!     <div style="background-color: #6A1B9A"></div>
//!     <div style="background-color: #4A148C"></div>
//!     <div style="background-color: #EA80FC"></div>
//!     <div style="background-color: #E040FB"></div>
//!     <div style="background-color: #D500F9"></div>
//!     <div style="background-color: #AA00FF"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`DEEP_PURPLE`]</div>
//!     <div style="background-color: #EDE7F6"></div>
//!     <div style="background-color: #D1C4E9"></div>
//!     <div style="background-color: #B39DDB"></div>
//!     <div style="background-color: #9575CD"></div>
//!     <div style="background-color: #7E57C2"></div>
//!     <div style="background-color: #673AB7"></div>
//!     <div style="background-color: #5E35B1"></div>
//!     <div style="background-color: #512DA8"></div>
//!     <div style="background-color: #4527A0"></div>
//!     <div style="background-color: #311B92"></div>
//!     <div style="background-color: #B388FF"></div>
//!     <div style="background-color: #7C4DFF"></div>
//!     <div style="background-color: #651FFF"></div>
//!     <div style="background-color: #6200EA"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`INDIGO`]</div>
//!     <div style="background-color: #E8EAF6"></div>
//!     <div style="background-color: #C5CAE9"></div>
//!     <div style="background-color: #9FA8DA"></div>
//!     <div style="background-color: #7986CB"></div>
//!     <div style="background-color: #5C6BC0"></div>
//!     <div style="background-color: #3F51B5"></div>
//!     <div style="background-color: #3949AB"></div>
//!     <div style="background-color: #303F9F"></div>
//!     <div style="background-color: #283593"></div>
//!     <div style="background-color: #1A237E"></div>
//!     <div style="background-color: #8C9EFF"></div>
//!     <div style="background-color: #536DFE"></div>
//!     <div style="background-color: #3D5AFE"></div>
//!     <div style="background-color: #304FFE"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`BLUE`]</div>
//!     <div style="background-color: #E3F2FD"></div>
//!     <div style="background-color: #BBDEFB"></div>
//!     <div style="background-color: #90CAF9"></div>
//!     <div style="background-color: #64B5F6"></div>
//!     <div style="background-color: #42A5F5"></div>
//!     <div style="background-color: #2196F3"></div>
//!     <div style="background-color: #1E88E5"></div>
//!     <div style="background-color: #1976D2"></div>
//!     <div style="background-color: #1565C0"></div>
//!     <div style="background-color: #0D47A1"></div>
//!     <div style="background-color: #82B1FF"></div>
//!     <div style="background-color: #448AFF"></div>
//!     <div style="background-color: #2979FF"></div>
//!     <div style="background-color: #2962FF"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`LIGHT_BLUE`]</div>
//!     <div style="background-color: #E1F5FE"></div>
//!     <div style="background-color: #B3E5FC"></div>
//!     <div style="background-color: #81D4FA"></div>
//!     <div style="background-color: #4FC3F7"></div>
//!     <div style="background-color: #29B6F6"></div>
//!     <div style="background-color: #03A9F4"></div>
//!     <div style="background-color: #039BE5"></div>
//!     <div style="background-color: #0288D1"></div>
//!     <div style="background-color: #0277BD"></div>
//!     <div style="background-color: #01579B"></div>
//!     <div style="background-color: #80D8FF"></div>
//!     <div style="background-color: #40C4FF"></div>
//!     <div style="background-color: #00B0FF"></div>
//!     <div style="background-color: #0091EA"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`CYAN`]</div>
//!     <div style="background-color: #E0F7FA"></div>
//!     <div style="background-color: #B2EBF2"></div>
//!     <div style="background-color: #80DEEA"></div>
//!     <div style="background-color: #4DD0E1"></div>
//!     <div style="background-color: #26C6DA"></div>
//!     <div style="background-color: #00BCD4"></div>
//!     <div style="background-color: #00ACC1"></div>
//!     <div style="background-color: #0097A7"></div>
//!     <div style="background-color: #00838F"></div>
//!     <div style="background-color: #006064"></div>
//!     <div style="background-color: #84FFFF"></div>
//!     <div style="background-color: #18FFFF"></div>
//!     <div style="background-color: #00E5FF"></div>
//!     <div style="background-color: #00B8D4"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`TEAL`]</div>
//!     <div style="background-color: #E0F2F1"></div>
//!     <div style="background-color: #B2DFDB"></div>
//!     <div style="background-color: #80CBC4"></div>
//!     <div style="background-color: #4DB6AC"></div>
//!     <div style="background-color: #26A69A"></div>
//!     <div style="background-color: #009688"></div>
//!     <div style="background-color: #00897B"></div>
//!     <div style="background-color: #00796B"></div>
//!     <div style="background-color: #00695C"></div>
//!     <div style="background-color: #004D40"></div>
//!     <div style="background-color: #A7FFEB"></div>
//!     <div style="background-color: #64FFDA"></div>
//!     <div style="background-color: #1DE9B6"></div>
//!     <div style="background-color: #00BFA5"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`GREEN`]</div>
//!     <div style="background-color: #E8F5E9"></div>
//!     <div style="background-color: #C8E6C9"></div>
//!     <div style="background-color: #A5D6A7"></div>
//!     <div style="background-color: #81C784"></div>
//!     <div style="background-color: #66BB6A"></div>
//!     <div style="background-color: #4CAF50"></div>
//!     <div style="background-color: #43A047"></div>
//!     <div style="background-color: #388E3C"></div>
//!     <div style="background-color: #2E7D32"></div>
//!     <div style="background-color: #1B5E20"></div>
//!     <div style="background-color: #B9F6CA"></div>
//!     <div style="background-color: #69F0AE"></div>
//!     <div style="background-color: #00E676"></div>
//!     <div style="background-color: #00C853"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`LIGHT_GREEN`]</div>
//!     <div style="background-color: #F1F8E9"></div>
//!     <div style="background-color: #DCEDC8"></div>
//!     <div style="background-color: #C5E1A5"></div>
//!     <div style="background-color: #AED581"></div>
//!     <div style="background-color: #9CCC65"></div>
//!     <div style="background-color: #8BC34A"></div>
//!     <div style="background-color: #7CB342"></div>
//!     <div style="background-color: #689F38"></div>
//!     <div style="background-color: #558B2F"></div>
//!     <div style="background-color: #33691E"></div>
//!     <div style="background-color: #CCFF90"></div>
//!     <div style="background-color: #B2FF59"></div>
//!     <div style="background-color: #76FF03"></div>
//!     <div style="background-color: #64DD17"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`LIME`]</div>
//!     <div style="background-color: #F9FBE7"></div>
//!     <div style="background-color: #F0F4C3"></div>
//!     <div style="background-color: #E6EE9C"></div>
//!     <div style="background-color: #DCE775"></div>
//!     <div style="background-color: #D4E157"></div>
//!     <div style="background-color: #CDDC39"></div>
//!     <div style="background-color: #C0CA33"></div>
//!     <div style="background-color: #AFB42B"></div>
//!     <div style="background-color: #9E9D24"></div>
//!     <div style="background-color: #827717"></div>
//!     <div style="background-color: #F4FF81"></div>
//!     <div style="background-color: #EEFF41"></div>
//!     <div style="background-color: #C6FF00"></div>
//!     <div style="background-color: #AEEA00"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`YELLOW`]</div>
//!     <div style="background-color: #FFFDE7"></div>
//!     <div style="background-color: #FFF9C4"></div>
//!     <div style="background-color: #FFF59D"></div>
//!     <div style="background-color: #FFF176"></div>
//!     <div style="background-color: #FFEE58"></div>
//!     <div style="background-color: #FFEB3B"></div>
//!     <div style="background-color: #FDD835"></div>
//!     <div style="background-color: #FBC02D"></div>
//!     <div style="background-color: #F9A825"></div>
//!     <div style="background-color: #F57F17"></div>
//!     <div style="background-color: #FFFF8D"></div>
//!     <div style="background-color: #FFFF00"></div>
//!     <div style="background-color: #FFEA00"></div>
//!     <div style="background-color: #FFD600"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`AMBER`]</div>
//!     <div style="background-color: #FFF8E1"></div>
//!     <div style="background-color: #FFECB3"></div>
//!     <div style="background-color: #FFE082"></div>
//!     <div style="background-color: #FFD54F"></div>
//!     <div style="background-color: #FFCA28"></div>
//!     <div style="background-color: #FFC107"></div>
//!     <div style="background-color: #FFB300"></div>
//!     <div style="background-color: #FFA000"></div>
//!     <div style="background-color: #FF8F00"></div>
//!     <div style="background-color: #FF6F00"></div>
//!     <div style="background-color: #FFE57F"></div>
//!     <div style="background-color: #FFD740"></div>
//!     <div style="background-color: #FFC400"></div>
//!     <div style="background-color: #FFAB00"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`ORANGE`]</div>
//!     <div style="background-color: #FFF3E0"></div>
//!     <div style="background-color: #FFE0B2"></div>
//!     <div style="background-color: #FFCC80"></div>
//!     <div style="background-color: #FFB74D"></div>
//!     <div style="background-color: #FFA726"></div>
//!     <div style="background-color: #FF9800"></div>
//!     <div style="background-color: #FB8C00"></div>
//!     <div style="background-color: #F57C00"></div>
//!     <div style="background-color: #EF6C00"></div>
//!     <div style="background-color: #E65100"></div>
//!     <div style="background-color: #FFD180"></div>
//!     <div style="background-color: #FFAB40"></div>
//!     <div style="background-color: #FF9100"></div>
//!     <div style="background-color: #FF6D00"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`DEEP_ORANGE`]</div>
//!     <div style="background-color: #FBE9E7"></div>
//!     <div style="background-color: #FFCCBC"></div>
//!     <div style="background-color: #FFAB91"></div>
//!     <div style="background-color: #FF8A65"></div>
//!     <div style="background-color: #FF7043"></div>
//!     <div style="background-color: #FF5722"></div>
//!     <div style="background-color: #F4511E"></div>
//!     <div style="background-color: #E64A19"></div>
//!     <div style="background-color: #D84315"></div>
//!     <div style="background-color: #BF360C"></div>
//!     <div style="background-color: #FF9E80"></div>
//!     <div style="background-color: #FF6E40"></div>
//!     <div style="background-color: #FF3D00"></div>
//!     <div style="background-color: #DD2C00"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`BROWN`]</div>
//!     <div style="background-color: #EFEBE9"></div>
//!     <div style="background-color: #D7CCC8"></div>
//!     <div style="background-color: #BCAAA4"></div>
//!     <div style="background-color: #A1887F"></div>
//!     <div style="background-color: #8D6E63"></div>
//!     <div style="background-color: #795548"></div>
//!     <div style="background-color: #6D4C41"></div>
//!     <div style="background-color: #5D4037"></div>
//!     <div style="background-color: #4E342E"></div>
//!     <div style="background-color: #3E2723"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`GRAY`]</div>
//!     <div style="background-color: #FAFAFA"></div>
//!     <div style="background-color: #F5F5F5"></div>
//!     <div style="background-color: #EEEEEE"></div>
//!     <div style="background-color: #E0E0E0"></div>
//!     <div style="background-color: #BDBDBD"></div>
//!     <div style="background-color: #9E9E9E"></div>
//!     <div style="background-color: #757575"></div>
//!     <div style="background-color: #616161"></div>
//!     <div style="background-color: #424242"></div>
//!     <div style="background-color: #212121"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`BLUE_GRAY`]</div>
//!     <div style="background-color: #ECEFF1"></div>
//!     <div style="background-color: #CFD8DC"></div>
//!     <div style="background-color: #B0BEC5"></div>
//!     <div style="background-color: #90A4AE"></div>
//!     <div style="background-color: #78909C"></div>
//!     <div style="background-color: #607D8B"></div>
//!     <div style="background-color: #546E7A"></div>
//!     <div style="background-color: #455A64"></div>
//!     <div style="background-color: #37474F"></div>
//!     <div style="background-color: #263238"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`BLACK`]</div>
//!     <div class="bw" style="width: 350px; background-color: #000000"></div>
//! </div>
//! <div class="color">
//!     <div class="name">
//!
//! [`WHITE`]</div>
//!     <div style="width: 350px; background-color: #FFFFFF"></div>
//! </div>
//! </div>
//! </div>
//!
//! # Example
//!
//! ```rust
//! use ratatui_core::style::Color;
//! use ratatui_core::style::palette::material::{BLUE, RED};
//!
//! assert_eq!(RED.c500, Color::Rgb(244, 67, 54));
//! assert_eq!(BLUE.c500, Color::Rgb(33, 150, 243));
//! ```
//!
//! [`matdesign-color` crate]: https://crates.io/crates/matdesign-color

use crate::style::Color;

/// A palette of colors for use in Material design with accent colors
///
/// This is a collection of colors that are used in Material design. They consist of a set of
/// colors from 50 to 900, and a set of accent colors from 100 to 700.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AccentedPalette {
    pub c50: Color,
    pub c100: Color,
    pub c200: Color,
    pub c300: Color,
    pub c400: Color,
    pub c500: Color,
    pub c600: Color,
    pub c700: Color,
    pub c800: Color,
    pub c900: Color,
    pub a100: Color,
    pub a200: Color,
    pub a400: Color,
    pub a700: Color,
}

/// A palette of colors for use in Material design without accent colors
///
/// This is a collection of colors that are used in Material design. They consist of a set of
/// colors from 50 to 900.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NonAccentedPalette {
    pub c50: Color,
    pub c100: Color,
    pub c200: Color,
    pub c300: Color,
    pub c400: Color,
    pub c500: Color,
    pub c600: Color,
    pub c700: Color,
    pub c800: Color,
    pub c900: Color,
}

impl AccentedPalette {
    /// Create a new `AccentedPalette` from the given variants
    ///
    /// The variants should be in the format [0x00RRGGBB, ...]
    pub const fn from_variants(variants: [u32; 14]) -> Self {
        Self {
            c50: Color::from_u32(variants[0]),
            c100: Color::from_u32(variants[1]),
            c200: Color::from_u32(variants[2]),
            c300: Color::from_u32(variants[3]),
            c400: Color::from_u32(variants[4]),
            c500: Color::from_u32(variants[5]),
            c600: Color::from_u32(variants[6]),
            c700: Color::from_u32(variants[7]),
            c800: Color::from_u32(variants[8]),
            c900: Color::from_u32(variants[9]),
            a100: Color::from_u32(variants[10]),
            a200: Color::from_u32(variants[11]),
            a400: Color::from_u32(variants[12]),
            a700: Color::from_u32(variants[13]),
        }
    }
}

impl NonAccentedPalette {
    /// Create a new `NonAccented` from the given variants
    ///
    /// The variants should be in the format [0x00RRGGBB, ...]
    pub const fn from_variants(variants: [u32; 10]) -> Self {
        Self {
            c50: Color::from_u32(variants[0]),
            c100: Color::from_u32(variants[1]),
            c200: Color::from_u32(variants[2]),
            c300: Color::from_u32(variants[3]),
            c400: Color::from_u32(variants[4]),
            c500: Color::from_u32(variants[5]),
            c600: Color::from_u32(variants[6]),
            c700: Color::from_u32(variants[7]),
            c800: Color::from_u32(variants[8]),
            c900: Color::from_u32(variants[9]),
        }
    }
}

// Accented palettes

pub const RED: AccentedPalette = AccentedPalette::from_variants(variants::RED);
pub const PINK: AccentedPalette = AccentedPalette::from_variants(variants::PINK);
pub const PURPLE: AccentedPalette = AccentedPalette::from_variants(variants::PURPLE);
pub const DEEP_PURPLE: AccentedPalette = AccentedPalette::from_variants(variants::DEEP_PURPLE);
pub const INDIGO: AccentedPalette = AccentedPalette::from_variants(variants::INDIGO);
pub const BLUE: AccentedPalette = AccentedPalette::from_variants(variants::BLUE);
pub const LIGHT_BLUE: AccentedPalette = AccentedPalette::from_variants(variants::LIGHT_BLUE);
pub const CYAN: AccentedPalette = AccentedPalette::from_variants(variants::CYAN);
pub const TEAL: AccentedPalette = AccentedPalette::from_variants(variants::TEAL);
pub const GREEN: AccentedPalette = AccentedPalette::from_variants(variants::GREEN);
pub const LIGHT_GREEN: AccentedPalette = AccentedPalette::from_variants(variants::LIGHT_GREEN);
pub const LIME: AccentedPalette = AccentedPalette::from_variants(variants::LIME);
pub const YELLOW: AccentedPalette = AccentedPalette::from_variants(variants::YELLOW);
pub const AMBER: AccentedPalette = AccentedPalette::from_variants(variants::AMBER);
pub const ORANGE: AccentedPalette = AccentedPalette::from_variants(variants::ORANGE);
pub const DEEP_ORANGE: AccentedPalette = AccentedPalette::from_variants(variants::DEEP_ORANGE);

// Unaccented palettes
pub const BROWN: NonAccentedPalette = NonAccentedPalette::from_variants(variants::BROWN);
pub const GRAY: NonAccentedPalette = NonAccentedPalette::from_variants(variants::GRAY);
pub const BLUE_GRAY: NonAccentedPalette = NonAccentedPalette::from_variants(variants::BLUE_GRAY);

// Black and white included for completeness
pub const BLACK: Color = Color::from_u32(0x000000);
pub const WHITE: Color = Color::from_u32(0xFFFFFF);

mod variants {
    pub const RED: [u32; 14] = [
        0xFFEBEE, 0xFFCDD2, 0xEF9A9A, 0xE57373, 0xEF5350, 0xF44336, 0xE53935, 0xD32F2F, 0xC62828,
        0xB71C1C, 0xFF8A80, 0xFF5252, 0xFF1744, 0xD50000,
    ];
    pub const PINK: [u32; 14] = [
        0xFCE4EC, 0xF8BBD0, 0xF48FB1, 0xF06292, 0xEC407A, 0xE91E63, 0xD81B60, 0xC2185B, 0xAD1457,
        0x880E4F, 0xFF80AB, 0xFF4081, 0xF50057, 0xC51162,
    ];
    pub const PURPLE: [u32; 14] = [
        0xF3E5F5, 0xE1BEE7, 0xCE93D8, 0xBA68C8, 0xAB47BC, 0x9C27B0, 0x8E24AA, 0x7B1FA2, 0x6A1B9A,
        0x4A148C, 0xEA80FC, 0xE040FB, 0xD500F9, 0xAA00FF,
    ];
    pub const DEEP_PURPLE: [u32; 14] = [
        0xEDE7F6, 0xD1C4E9, 0xB39DDB, 0x9575CD, 0x7E57C2, 0x673AB7, 0x5E35B1, 0x512DA8, 0x4527A0,
        0x311B92, 0xB388FF, 0x7C4DFF, 0x651FFF, 0x6200EA,
    ];
    pub const INDIGO: [u32; 14] = [
        0xE8EAF6, 0xC5CAE9, 0x9FA8DA, 0x7986CB, 0x5C6BC0, 0x3F51B5, 0x3949AB, 0x303F9F, 0x283593,
        0x1A237E, 0x8C9EFF, 0x536DFE, 0x3D5AFE, 0x304FFE,
    ];
    pub const BLUE: [u32; 14] = [
        0xE3F2FD, 0xBBDEFB, 0x90CAF9, 0x64B5F6, 0x42A5F5, 0x2196F3, 0x1E88E5, 0x1976D2, 0x1565C0,
        0x0D47A1, 0x82B1FF, 0x448AFF, 0x2979FF, 0x2962FF,
    ];
    pub const LIGHT_BLUE: [u32; 14] = [
        0xE1F5FE, 0xB3E5FC, 0x81D4FA, 0x4FC3F7, 0x29B6F6, 0x03A9F4, 0x039BE5, 0x0288D1, 0x0277BD,
        0x01579B, 0x80D8FF, 0x40C4FF, 0x00B0FF, 0x0091EA,
    ];
    pub const CYAN: [u32; 14] = [
        0xE0F7FA, 0xB2EBF2, 0x80DEEA, 0x4DD0E1, 0x26C6DA, 0x00BCD4, 0x00ACC1, 0x0097A7, 0x00838F,
        0x006064, 0x84FFFF, 0x18FFFF, 0x00E5FF, 0x00B8D4,
    ];
    pub const TEAL: [u32; 14] = [
        0xE0F2F1, 0xB2DFDB, 0x80CBC4, 0x4DB6AC, 0x26A69A, 0x009688, 0x00897B, 0x00796B, 0x00695C,
        0x004D40, 0xA7FFEB, 0x64FFDA, 0x1DE9B6, 0x00BFA5,
    ];
    pub const GREEN: [u32; 14] = [
        0xE8F5E9, 0xC8E6C9, 0xA5D6A7, 0x81C784, 0x66BB6A, 0x4CAF50, 0x43A047, 0x388E3C, 0x2E7D32,
        0x1B5E20, 0xB9F6CA, 0x69F0AE, 0x00E676, 0x00C853,
    ];
    pub const LIGHT_GREEN: [u32; 14] = [
        0xF1F8E9, 0xDCEDC8, 0xC5E1A5, 0xAED581, 0x9CCC65, 0x8BC34A, 0x7CB342, 0x689F38, 0x558B2F,
        0x33691E, 0xCCFF90, 0xB2FF59, 0x76FF03, 0x64DD17,
    ];
    pub const LIME: [u32; 14] = [
        0xF9FBE7, 0xF0F4C3, 0xE6EE9C, 0xDCE775, 0xD4E157, 0xCDDC39, 0xC0CA33, 0xAFB42B, 0x9E9D24,
        0x827717, 0xF4FF81, 0xEEFF41, 0xC6FF00, 0xAEEA00,
    ];
    pub const YELLOW: [u32; 14] = [
        0xFFFDE7, 0xFFF9C4, 0xFFF59D, 0xFFF176, 0xFFEE58, 0xFFEB3B, 0xFDD835, 0xFBC02D, 0xF9A825,
        0xF57F17, 0xFFFF8D, 0xFFFF00, 0xFFEA00, 0xFFD600,
    ];
    pub const AMBER: [u32; 14] = [
        0xFFF8E1, 0xFFECB3, 0xFFE082, 0xFFD54F, 0xFFCA28, 0xFFC107, 0xFFB300, 0xFFA000, 0xFF8F00,
        0xFF6F00, 0xFFE57F, 0xFFD740, 0xFFC400, 0xFFAB00,
    ];
    pub const ORANGE: [u32; 14] = [
        0xFFF3E0, 0xFFE0B2, 0xFFCC80, 0xFFB74D, 0xFFA726, 0xFF9800, 0xFB8C00, 0xF57C00, 0xEF6C00,
        0xE65100, 0xFFD180, 0xFFAB40, 0xFF9100, 0xFF6D00,
    ];
    pub const DEEP_ORANGE: [u32; 14] = [
        0xFBE9E7, 0xFFCCBC, 0xFFAB91, 0xFF8A65, 0xFF7043, 0xFF5722, 0xF4511E, 0xE64A19, 0xD84315,
        0xBF360C, 0xFF9E80, 0xFF6E40, 0xFF3D00, 0xDD2C00,
    ];
    pub const BROWN: [u32; 10] = [
        0xEFEBE9, 0xD7CCC8, 0xBCAAA4, 0xA1887F, 0x8D6E63, 0x795548, 0x6D4C41, 0x5D4037, 0x4E342E,
        0x3E2723,
    ];
    pub const GRAY: [u32; 10] = [
        0xFAFAFA, 0xF5F5F5, 0xEEEEEE, 0xE0E0E0, 0xBDBDBD, 0x9E9E9E, 0x757575, 0x616161, 0x424242,
        0x212121,
    ];
    pub const BLUE_GRAY: [u32; 10] = [
        0xECEFF1, 0xCFD8DC, 0xB0BEC5, 0x90A4AE, 0x78909C, 0x607D8B, 0x546E7A, 0x455A64, 0x37474F,
        0x263238,
    ];
}
