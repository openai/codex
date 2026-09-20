//! Deterministic surface projection and Braille rasterization of the Codex logo morph.
//!
//! Fields and depth use Float32 sampling; all other math uses Float64.

use std::f64::consts::TAU;

use super::geometry::EXTENT;
use super::geometry::FIELDS;
use super::geometry::GRID;
use super::geometry::STEP;
use super::lighting::Lighting;

pub(super) const MAX_COLUMNS: u16 = 60;
pub(super) const MAX_ROWS: u16 = 24;
const DOT_BITS: [u8; 8] = [1, 8, 2, 16, 4, 32, 64, 128];
// First three mulberry32 samples with the reference's seed 3371.
const HALF_DEPTH: f64 = 0.1642880353482906;
const WOBBLE: f64 = 0.11242630996974184;
const GRAIN_PHASE: f64 = 3.8990964158506114;

fn smooth(value: f64) -> f64 {
    let t = value.clamp(/*min*/ 0.0, /*max*/ 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Cell {
    pub(super) dots: u8,
    pub(super) rgb: u32,
}

pub(super) struct Renderer {
    shape: Vec<f32>,
    depth: Vec<f32>,
    colors: Vec<u32>,
    cells: Vec<Cell>,
}

impl Default for Renderer {
    fn default() -> Self {
        let samples = usize::from(MAX_COLUMNS) * usize::from(MAX_ROWS) * 8;
        Self {
            shape: vec![0.0; GRID * GRID],
            depth: vec![f32::NEG_INFINITY; samples],
            colors: vec![0; samples],
            cells: vec![Cell::default(); samples / 8],
        }
    }
}

impl Renderer {
    pub(super) fn frame(
        &mut self,
        columns: u16,
        rows: u16,
        phase: f64,
        light: &Lighting,
    ) -> &[Cell] {
        assert!(columns <= MAX_COLUMNS && rows <= MAX_ROWS);
        let columns = usize::from(columns);
        let rows = usize::from(rows);
        let phase = phase.rem_euclid(/*rhs*/ 1.0);
        let second = if phase >= 0.5 { 1.0 } else { 0.0 };
        let spin = smooth((phase * 2.0 - second) / 0.82);
        let morph = smooth((spin - 2.0 / 3.0) * 3.0);
        let rotation = (second + spin) * TAU;
        let blend = if second == 1.0 { 1.0 - morph } else { morph };
        let (sx, cx) = (rotation.sin() * WOBBLE).sin_cos();
        let (sy, cy) = rotation.sin_cos();
        let (sz, cz) = ((phase * TAU).sin() * 0.045).sin_cos();
        let bevel = 0.075 + blend * 0.012;
        let dot_columns = columns * 2;
        let dot_rows = rows * 4;
        let dot_width = 800.0 / dot_columns as f64;
        let dot_height = 550.0 / dot_rows as f64;
        self.depth.fill(f32::NEG_INFINITY);
        for (i, value) in self.shape.iter_mut().enumerate() {
            *value =
                (f64::from(FIELDS[0][i]) * (1.0 - blend) + f64::from(FIELDS[1][i]) * blend) as f32;
        }
        let mut project = |[x, y, mut z]: [f64; 3], [mut nx, mut ny, mut nz]: [f64; 3]| {
            let u = x * 2.2 + y * 0.7;
            let v = y * 2.8 - x * 0.4;
            z += u.sin() * 0.075 + v.sin() * 0.04;
            nx -= nz * (u.cos() * 0.165 - v.cos() * 0.016);
            ny -= nz * (u.cos() * 0.0525 + v.cos() * 0.112);
            let length = nx.hypot(ny).hypot(nz);
            let length = if length == 0.0 { 1.0 } else { length };
            nx /= length;
            ny /= length;
            nz /= length;
            let x1 = x * cy + z * sy;
            let z1 = -x * sy + z * cy;
            let y2 = y * cx - z1 * sx;
            let z2 = y * sx + z1 * cx;
            let x3 = x1 * cz - y2 * sz;
            let y3 = x1 * sz + y2 * cz;
            let perspective = 4.4 / (4.4 - z2);
            let column = ((400.0 + x3 * 206.0 * perspective) / dot_width).floor() as isize;
            let row = ((275.0 + y3 * 206.0 * perspective) / dot_height).floor() as isize;
            if column < 0 || column >= dot_columns as isize || row < 0 || row >= dot_rows as isize {
                return;
            }
            let i = row as usize * dot_columns + column as usize;
            if z2 <= f64::from(self.depth[i]) {
                return;
            }
            let nx1 = nx * cy + nz * sy;
            let nz1 = -nx * sy + nz * cy;
            let ny2 = ny * cx - nz1 * sx;
            let nz2 = ny * sx + nz1 * cx;
            self.depth[i] = z2 as f32;
            self.colors[i] = light.shade(
                [nx1 * cz - ny2 * sz, nx1 * sz + ny2 * cz, nz2],
                z2,
                (x * 23.0 + y * 19.0 + GRAIN_PHASE).sin() * 0.01,
            );
        };
        for row in 1..GRID - 1 {
            for column in 1..GRID - 1 {
                let i = row * GRID + column;
                let d = f64::from(self.shape[i]);
                if d < -STEP {
                    continue;
                }
                let x = column as f64 * STEP - EXTENT;
                let y = row as f64 * STEP - EXTENT;
                let dx = f64::from(self.shape[i + 1]) - f64::from(self.shape[i - 1]);
                let dy = f64::from(self.shape[i + GRID]) - f64::from(self.shape[i - GRID]);
                let length = dx.hypot(dy);
                let length = if length == 0.0 { 1.0 } else { length };
                let gx = dx / length;
                let gy = dy / length;
                if d > 0.0 {
                    let edge = (1.0 - d / bevel).clamp(/*min*/ 0.0, /*max*/ 1.0);
                    let nz = (1.0 - edge * edge).sqrt();
                    let z = HALF_DEPTH - bevel + bevel * nz;
                    project([x, y, z], [-gx * edge, -gy * edge, nz]);
                    project([x, y, -z], [-gx * edge, -gy * edge, -nz]);
                }
                if d.abs() < STEP * 0.8 {
                    let side_depth = HALF_DEPTH - bevel;
                    let layers = (side_depth * 2.0 / STEP).ceil() as usize;
                    for layer in 0..=layers {
                        project(
                            [
                                x - gx * d,
                                y - gy * d,
                                -side_depth + layer as f64 / layers as f64 * side_depth * 2.0,
                            ],
                            [-gx, -gy, 0.0],
                        );
                    }
                }
            }
        }
        self.cells.fill(Cell::default());
        for row in 0..rows {
            for column in 0..columns {
                let (mut dots, mut count, mut rgb) = (0, 0, [0; 3]);
                for (point, bit) in DOT_BITS.into_iter().enumerate() {
                    let i = (row * 4 + point / 2) * dot_columns + column * 2 + point % 2;
                    if self.depth[i].is_finite() {
                        dots |= bit;
                        let channels = self.colors[i].to_be_bytes();
                        for channel in 0..3 {
                            rgb[channel] += u32::from(channels[channel + 1]);
                        }
                        count += 1;
                    }
                }
                let rgb = if count == 0 {
                    0
                } else {
                    let channels = rgb.map(|value| ((value + count / 2) / count) as u8);
                    u32::from_be_bytes([0, channels[0], channels[1], channels[2]])
                };
                self.cells[row * columns + column] = Cell { dots, rgb };
            }
        }
        &self.cells[..rows * columns]
    }
}
