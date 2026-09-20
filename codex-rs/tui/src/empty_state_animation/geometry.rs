//! Cached signed distance fields of the original vector marks, preserving their cutouts.

use std::sync::LazyLock;

use super::paths;

pub(super) const GRID: usize = 160;
pub(super) const EXTENT: f64 = 1.08;
pub(super) const STEP: f64 = EXTENT * 2.0 / (GRID - 1) as f64;
pub(super) static FIELDS: LazyLock<[Vec<f32>; 2]> =
    LazyLock::new(|| [field(paths::CODEX), field(paths::OPENAI)]);

struct Path<'a> {
    remaining: &'a str,
}

impl Path<'_> {
    #[expect(
        clippy::expect_used,
        reason = "coordinates come only from the embedded, reference-tested SVG paths"
    )]
    fn number(&mut self) -> f64 {
        self.remaining = self.remaining.trim_start();
        let end = self.remaining[1..]
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .map_or(self.remaining.len(), |i| i + 1);
        let (number, remaining) = self.remaining.split_at(end);
        self.remaining = remaining;
        number.parse().expect("embedded logo coordinate")
    }

    fn point(&mut self) -> [f64; 2] {
        [self.number(), self.number()]
    }
}

fn field((path, bounds): (&str, [f64; 4])) -> Vec<f32> {
    let mut path = Path { remaining: path };
    let mut point = [0.0; 2];
    let mut start = point;
    let mut segments = Vec::new();
    while let Some(command) = path.remaining.chars().next() {
        path.remaining = &path.remaining[1..];
        let next = match command {
            'M' => {
                point = path.point();
                start = point;
                continue;
            }
            'L' => path.point(),
            'H' => [path.number(), point[1]],
            'V' => [point[0], path.number()],
            'Z' => start,
            'C' => {
                let origin = point;
                let a = path.point();
                let b = path.point();
                let end = path.point();
                for step in 1..=12 {
                    let t = f64::from(step) / 12.0;
                    let u = 1.0 - t;
                    let next = std::array::from_fn(|i| {
                        u.powi(/*n*/ 3) * origin[i]
                            + 3.0 * u * u * t * a[i]
                            + 3.0 * u * t * t * b[i]
                            + t.powi(/*n*/ 3) * end[i]
                    });
                    segments.push((point, next));
                    point = next;
                }
                continue;
            }
            _ => unreachable!("embedded paths contain only M/L/H/V/C/Z"),
        };
        segments.push((point, next));
        point = next;
    }
    let scale = (bounds[2] - bounds[0]).max(bounds[3] - bounds[1]) / 2.0;
    for (a, b) in &mut segments {
        for axis in 0..2 {
            let center = (bounds[axis] + bounds[axis + 2]) / 2.0;
            a[axis] = (a[axis] - center) / scale;
            b[axis] = (b[axis] - center) / scale;
        }
    }
    let mut inside = vec![false; GRID * GRID];
    for row in 0..GRID {
        let y = row as f64 * STEP - EXTENT;
        let mut crossings = segments
            .iter()
            .filter(|&(a, b)| (a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y))
            .map(|(a, b)| {
                (
                    a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]),
                    if b[1] > a[1] { 1 } else { -1 },
                )
            })
            .collect::<Vec<_>>();
        crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut cursor, mut winding) = (0, 0);
        for column in 0..GRID {
            let x = column as f64 * STEP - EXTENT;
            while cursor < crossings.len() && crossings[cursor].0 <= x {
                winding += crossings[cursor].1;
                cursor += 1;
            }
            inside[row * GRID + column] = winding != 0;
        }
    }
    let mut distance = vec![GRID as f32; GRID * GRID];
    for row in 1..GRID - 1 {
        for column in 1..GRID - 1 {
            let i = row * GRID + column;
            if [i - 1, i + 1, i - GRID, i + GRID]
                .into_iter()
                .any(|j| inside[i] != inside[j])
            {
                distance[i] = 0.5;
            }
        }
    }
    for reverse in [false, true] {
        for row in 1..GRID - 1 {
            for column in 1..GRID - 1 {
                let i = if reverse {
                    (GRID - 1 - row) * GRID + GRID - 1 - column
                } else {
                    row * GRID + column
                };
                let neighbors = if reverse {
                    [i + 1, i + GRID, i + GRID - 1, i + GRID + 1]
                } else {
                    [i - 1, i - GRID, i - GRID - 1, i - GRID + 1]
                };
                let value = neighbors
                    .into_iter()
                    .zip([1.0, 1.0, std::f64::consts::SQRT_2, std::f64::consts::SQRT_2])
                    .fold(f64::from(distance[i]), |d, (j, step)| {
                        d.min(f64::from(distance[j]) + step)
                    });
                distance[i] = value as f32;
            }
        }
    }
    for (i, distance) in distance.iter_mut().enumerate() {
        *distance = (f64::from(*distance) * STEP * if inside[i] { 1.0 } else { -1.0 }) as f32;
    }
    distance
}
