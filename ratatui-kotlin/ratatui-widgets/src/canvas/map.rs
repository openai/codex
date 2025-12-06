use ratatui_core::style::Color;
use strum::{Display, EnumString};

use crate::canvas::world::{WORLD_HIGH_RESOLUTION, WORLD_LOW_RESOLUTION};
use crate::canvas::{Painter, Shape};

/// Defines how many points are going to be used to draw a [`Map`].
///
/// You generally want a [high](MapResolution::High) resolution map.
#[derive(Debug, Default, Display, EnumString, Clone, Copy, Eq, PartialEq, Hash)]
pub enum MapResolution {
    /// A lesser resolution for the [`Map`] [`Shape`].
    ///
    /// Contains about 1000 points.
    #[default]
    Low,
    /// A higher resolution for the [`Map`] [`Shape`].
    ///
    /// Contains about 5000 points, you likely want to use [`Marker::Braille`] with this.
    ///
    /// [`Marker::Braille`]: (ratatui_core::symbols::Marker::Braille)
    High,
}

impl MapResolution {
    const fn data(self) -> &'static [(f64, f64)] {
        match self {
            Self::Low => &WORLD_LOW_RESOLUTION,
            Self::High => &WORLD_HIGH_RESOLUTION,
        }
    }
}

/// A world map
///
/// A world map can be rendered with different [resolutions](MapResolution) and [colors](Color).
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Map {
    /// The resolution of the map.
    ///
    /// This is the number of points used to draw the map.
    pub resolution: MapResolution,
    /// Map color
    ///
    /// This is the color of the points of the map.
    pub color: Color,
}

impl Shape for Map {
    fn draw(&self, painter: &mut Painter) {
        for (x, y) in self.resolution.data() {
            if let Some((x, y)) = painter.get_point(*x, *y) {
                painter.paint(x, y, self.color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::Rect;
    use ratatui_core::symbols::Marker;
    use ratatui_core::widgets::Widget;
    use strum::ParseError;

    use super::*;
    use crate::canvas::Canvas;

    #[test]
    fn map_resolution_to_string() {
        assert_eq!(MapResolution::Low.to_string(), "Low");
        assert_eq!(MapResolution::High.to_string(), "High");
    }

    #[test]
    fn map_resolution_from_str() {
        assert_eq!("Low".parse(), Ok(MapResolution::Low));
        assert_eq!("High".parse(), Ok(MapResolution::High));
        assert_eq!(
            "".parse::<MapResolution>(),
            Err(ParseError::VariantNotFound)
        );
    }

    #[test]
    fn default() {
        let map = Map::default();
        assert_eq!(map.resolution, MapResolution::Low);
        assert_eq!(map.color, Color::Reset);
    }

    #[test]
    fn draw_low() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 40));
        let canvas = Canvas::default()
            .marker(Marker::Dot)
            .x_bounds([-180.0, 180.0])
            .y_bounds([-90.0, 90.0])
            .paint(|context| {
                context.draw(&Map::default());
            });
        canvas.render(buffer.area, &mut buffer);
        let expected = Buffer::with_lines([
            "                                                                                ",
            "                               •                                                ",
            "               • •• •••••••• ••   ••••    •••••  ••• ••     •••                 ",
            "             •••••••••••••••       •      ••••      • •   •••••••     •••       ",
            "    • •••• ••••••••••••••• ••     ••  •     •••    ••  ••••    ••  ••••••• •••  ",
            "•••••     •••••••••••• •••• •  ••••••     •••• • ••• •••••                     •",
            "   ••  • •   •••• ••••••••  ••••   ••  • •• •  •••                        •• •••",
            "    •••• •••   •••••• •••••   •       •• ••••••                       • •••••   ",
            "•••••     •••     •  ••   ••         •••••••                          ••  •• •• ",
            "            ••    ••••  •••••          ••       •  • •                ••        ",
            "            •  •    •••••••           •• •••• ••• •• •  ••          • ••        ",
            "            •          ••             ••••••••• • ••             •••• •         ",
            "             ••       ••              • • • •• •                  •••••         ",
            "              ••   •••               •      ••••  •               • •           ",
            "               •  •   ••             •         ••  •• •           •             ",
            "    ••          • •••••••           •           •   •  •   •   •• •             ",
            "                 •••••••••          •           •• •   •  • •• •  ••            ",
            "                    ••  ••          •            •••     •   •••  ••            ",
            "                     •••  • •        •  •         •     ••  •••  •••            ",
            "                      •               •  ••                   • ••              ",
            "                   •  •     •••                • •            •••   •••         ",
            "                                •         •     •              • •    •••       ",
            "  •                                        •    • •                  • • •      ",
            "                       •       •                • •               ••• ••       •",
            "                        •      •          •    • ••              •      •   •   ",
            "                        •    •                   •               •       •      ",
            "                        •   •              •   •                    •           ",
            "                           ••               ••                   ••  ••  •   •  ",
            "                       •  •                                           •••    •• ",
            "                       •  •                                            ••   ••  ",
            "                       • •                                                      ",
            "                       •••••                                                    ",
            "                                                                                ",
            "                          ••                                                    ",
            "                         •••           •       • ••••• • •••• • • •• •• ••      ",
            "            •    • • ••••••        ••••••••• • ••      ••                  •••  ",
            "•    ••• •••• ••••   • •  • ••• • •                                        ••• •",
            "   •• •                •  ••  • ••                                         ••   ",
            "•      •                                                                      • ",
            "                                                                                ",
        ]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn draw_high() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 80, 40));
        let canvas = Canvas::default()
            .marker(Marker::Braille)
            .x_bounds([-180.0, 180.0])
            .y_bounds([-90.0, 90.0])
            .paint(|context| {
                context.draw(&Map {
                    resolution: MapResolution::High,
                    ..Default::default()
                });
            });
        canvas.render(buffer.area, &mut buffer);
        let expected = Buffer::with_lines([
            "                                                                                ",
            "                   ⢀⣀⣤⠄⠤⠤⣤⣀⡀⣀⣀⡄⠄⢄⣀⣄⡄⢀⡀                                          ",
            "             ⢀⣀⣤⠰⢤⣼⡯⢽⡟⣀⢶⣺⡛⠁       ⠈⢰⠃⠁    ⢖⣒⣾⡟⠂  ⠈⠛⠁        ⠺⢩⢖⡄                ",
            "            ⡬⢍⣿⣟⣿⣻⣿⣿⣿⡾⣯⡀⠈⠁⠁⢦      ⢀⡿       ⠈       ⢠⢶⠘⠋⡁⣀⢠⠤⠖⠘⠉⠁⠈⠼⡧⡄⣄⡀ ⢫⣗⠒⠆      ",
            "⣓  ⣠⠖⠓⠒⠢⠤⢄⠤⠶⠽⠽⣶⣃⣽⡮⣿⡷⣗⣤⡭⣍⢓⡄ ⠸⣷   ⢀⣀⠿⠇       ⢀⠔⠒⠲⠄⢄⢀⡀⢙⣑⡄⠴⡍⣟⠉          ⠑⠉⠉  ⠑⠐⠦⠤⣤⠤⢞",
            "⠶⢧⣗⢾⡆         ⠈⠈⠁⠈⠉⢀⣹⣶⣩⣽⣐⢮⠃ ⣇ ⢀⡔⠊ ⢰⣖⣲    ⢀⡐⠁⣰⠦ ⢲⣶⠛⠋    ⠐⠋                      ⡤",
            "  ⠉⣮⣀⣀⣴⡤⣠⡀         ⡎ ⠛⢫⠙⢫⢫  ⠈⠦⠼          ⡃⡀⢸⠼⣤⡄                        ⡀⣀⣀⡐⡶⣣⢤⠖⠉",
            "   ⢀⡽⠟⠃  ⠈⠱⡀       ⠙⠢⣀⣨⠆⠈⠁⢧⡀          ⣸⣷ ⢹⣷⣼⣸⠃                       ⢀⡐⢀ ⠁⡚⣨⠆   ",
            "          ⠘⢳⡀        ⠈⠾  ⣀⣀⣽         ⠸⢼⣇⡧⠋⠉⠁                          ⠉⣿  ⠢⠂    ",
            "           ⠈⢻           ⠜⢹⣵⠻⠇         ⠈⢻  ⢀⡀  ⢠⣠⡤ ⢀⢤                  ⢰⣯        ",
            "            ⢼          ⢀⣾⠛⠉          ⠐⡖⠒⡰⢺⣞⣵⡄⢀⣏⡭⣙⡄⢕⢫⡀             ⢀ ⢠⠖⢱⡿⠃       ",
            "            ⠸         ⠠⡎             ⠰⣅⣰⣃⣘⡣⡿⢻⡿⣁⣀  ⠸⣽             ⠐⣿⣽⣫ ⡸⡇        ",
            "             ⠳⣄       ⡰⠃             ⢀⠎⠉  ⢧⡀⣠⣛⠈⢻                  ⢻⠘⢺⡿⠚⠁        ",
            "              ⢻⣇  ⣠⠲⠖⢲⡇              ⡸     ⠉⠃⠈⠉⣿  ⢰⣆              ⢸ ⠈⠁          ",
            "              ⠈⢿⣆ ⡟  ⣘⣻             ⡸          ⢸⢇ ⠈⠯⢿⡒⠲⡀   ⢀⡀    ⣀⢾             ",
            "    ⠈⢳          ⠸⡀⢳⣠⢾⠉⢹⣦⣤⣀          ⡇           ⡿⡄  ⢰⠃ ⠑⡂ ⢠⠏⢣  ⣼⡮⠁⢈⡀            ",
            "                 ⠙⠲⢆⡿⢦⠈⠉⠁⠁          ⡇           ⠱⣇⣀⠼⠃   ⡃⢰⠃ ⠸⢶ ⠘⠄ ⢾⡁            ",
            "                    ⠙⣾⣀⡴⡶⢤⣤         ⢳            ⠻⠵⡆    ⠸⣸   ⢸⡳⡤⠃⢀⡾⣿            ",
            "                     ⠘⢻⠁  ⠈⠦⣄        ⢧⣀⣀⠤⣀        ⢐⠁    ⠈⠩⠆  ⣘⣧⠁ ⡸⡔⢿            ",
            "                      ⡸     ⢨         ⠁  ⠉⡇      ⢀⠎          ⢻⢿⠄⡴⢑⣧⡠⡄           ",
            "                      ⡇     ⠈⠋⠦⡄         ⠈⡆     ⢠⠃            ⢏⡇⢧⣼⣾⣧⣽⣿⠶⢤⡀⣤      ",
            "                      ⣇        ⠈⡇         ⢸     ⢸             ⠈⠶⣦⣄⣋⣁⡀⠸⣵⢠⣻⠋⠷⣄    ",
            "                      ⠰⡀       ⣰⠁         ⢘⠆    ⢸ ⢠⡀              ⠙⠋⢠⠦⡄⣷⠙⠃ ⠙    ",
            "⠄                      ⠣⡀      ⡃          ⢸     ⣸⢡⢾⠆               ⡞⠛⠘⢧⡏⡆   ⠸⠄ ⡤",
            "                        ⠱     ⢠⠃          ⠸⡀   ⢸⠁⢸⢨              ⡤⠚     ⠱⡀  ⢦  ⠁",
            "                        ⠅    ⡖⠉            ⡇   ⡜ ⠸⠔              ⡇       ⢳      ",
            "                        ⡇   ⢀⠃             ⢱⡀ ⢰⠃                 ⣇  ⢀⡀   ⢸      ",
            "                       ⢀⠃  ⡦⠏              ⠈⠷⠖⠃                  ⠾⠴⠊⠁⠹⣦  ⡞    ⣄ ",
            "                       ⢸  ⡤⠃                                          ⠘⢲⠖⠃    ⣽⡆",
            "                       ⢸ ⣸⠁                                            ⠈⠿   ⢀⢼⠏ ",
            "                       ⠞ ⡗                             ⣄                    ⠈⠋  ",
            "                       ⢧⡼⡁⠲⠂                                                    ",
            "                        ⠙⠉                                                      ",
            "                           ⡀                                                    ",
            "                         ⣴⠏⠁                      ⣀⡤⢤⣀⣀  ⢀⣀⣤⣀⣀⡴⣄⡤⢤⣀⠤⠤⠴⣄⣀⡀       ",
            "                 ⣀⣀    ⣠⣿⡍⣆          ⣠⣤⣤⠤⠴⠶⠖⠲⠤⠔⠛⠒⠉   ⠈⠨⣇⠖⠋              ⠈⠉⠓⠢⠤⢄  ",
            "     ⡀ ⣠⠤⠴⠒⠚⠛⠛⠒⠢⠤⠿⠙⠉⠉⠑⢋⣚⣉⠥⠚      ⢀⣀⡠⠟⠁                                      ⡴⠋  ",
            "   ⠐⠶⣛⣫⡤              ⠐⢏⣀⣤⣤ ⣴⣋⢇⢀⣮⡥                                         ⣴⠓   ",
            "⠤⠤⠤⠤⡀⣈⢣⣠⡄                 ⠉⠊⠉⠉⠉                                            ⠈⠓⠆⠤⠤",
            "                                                                                ",
        ]);
        assert_eq!(buffer, expected);
    }
}
