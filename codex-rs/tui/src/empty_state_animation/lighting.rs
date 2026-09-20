//! Fixed studio lighting, adapted to the terminal's foreground and background.

use crate::color::is_light;

pub(super) struct Lighting {
    pub(super) background: [f64; 3],
    pub(super) key: [f64; 3],
    pub(super) shadow: [f64; 3],
    pub(super) fill: [f64; 3],
    pub(super) rim: [f64; 3],
    pub(super) highlight: [f64; 3],
}

impl Lighting {
    pub(super) fn terminal(fg: (u8, u8, u8), bg: (u8, u8, u8)) -> Self {
        let background = [bg.0.into(), bg.1.into(), bg.2.into()];
        if is_light(bg) {
            let key = [fg.0.into(), fg.1.into(), fg.2.into()];
            Self {
                background,
                key,
                shadow: std::array::from_fn(|i| background[i] * 0.65 + key[i] * 0.35),
                fill: key,
                rim: key,
                highlight: key,
            }
        } else {
            Self {
                background,
                key: [210.0, 221.0, 235.0],
                shadow: [52.0, 71.0, 105.0],
                fill: [76.0, 118.0, 159.0],
                rim: [91.0, 196.0, 216.0],
                highlight: [255.0, 244.0, 218.0],
            }
        }
    }

    pub(super) fn shade(&self, [nx, ny, nz]: [f64; 3], depth: f64, grain: f64) -> u32 {
        let diffuse = (nx * -0.410 + ny * -0.564 + nz * 0.718 + grain)
            .clamp(/*min*/ 0.0, /*max*/ 1.0);
        let bounce = (nx * 0.55 + ny * 0.2 - nz * 0.35).clamp(/*min*/ 0.0, /*max*/ 1.0)
            * (1.0 - diffuse)
            * 0.38;
        let edge = (1.0 - nz.clamp(/*min*/ -1.0, /*max*/ 1.0).abs()).powf(/*n*/ 2.4)
            * (nx * 0.85 - ny * 0.38).clamp(/*min*/ 0.0, /*max*/ 1.0)
            * 0.82;
        let gloss = (nx * -0.220 + ny * -0.302 + nz * 0.928)
            .clamp(/*min*/ 0.0, /*max*/ 1.0)
            .powi(/*n*/ 18)
            * 0.96;
        let base = (1.0 - bounce) * (1.0 - edge) * (1.0 - gloss);
        let gain = (0.90 + depth * 0.18).clamp(/*min*/ 0.68, /*max*/ 1.0);
        let rgb: [u8; 3] = std::array::from_fn(|i| {
            let value = self.shadow[i] * (1.0 - diffuse) * base
                + self.key[i] * diffuse * base
                + self.fill[i] * bounce * (1.0 - edge) * (1.0 - gloss)
                + self.rim[i] * edge * (1.0 - gloss)
                + self.highlight[i] * gloss;
            (self.background[i] + (value - self.background[i]) * gain).round() as u8
        });
        u32::from_be_bytes([0, rgb[0], rgb[1], rgb[2]])
    }
}
