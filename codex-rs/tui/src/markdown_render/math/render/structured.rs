//! Small, bounded layouts for aligned equations and common optimization notation.
//! Cell parsing shares the parent parser's depth budget and preserves nested groups.
//! Aligned blocks compose with surrounding math at their first row's baseline.
//! Display annotations and sum limits are centered above or below their base.

use super::Layout;
use super::MAX_COLUMNS;
use super::MAX_ROWS;
use super::MathParser;
use super::SequenceEnd;
use crate::width::display_width;

enum AnnotationPosition {
    Above,
    Below,
}

impl Layout {
    fn annotated(self, annotation: Self, position: AnnotationPosition) -> Option<Self> {
        let width = self.width().max(annotation.width());
        if self.rows.len() + annotation.rows.len() > MAX_ROWS || width > MAX_COLUMNS {
            return None;
        }
        let baseline = match position {
            AnnotationPosition::Above => self.baseline + annotation.rows.len(),
            AnnotationPosition::Below => self.baseline,
        };
        let parts = match position {
            AnnotationPosition::Above => [annotation, self],
            AnnotationPosition::Below => [self, annotation],
        };
        let mut rows = Vec::new();
        for part in parts {
            let padding = " ".repeat((width - part.width()) / 2);
            rows.extend(part.rows.into_iter().map(|row| format!("{padding}{row}")));
        }
        Some(Self { rows, baseline })
    }
}

impl MathParser<'_> {
    // Single-line child layouts retain compact limits without flattening nested fractions.
    pub(super) fn compact_argument(&mut self) -> Option<Layout> {
        let stack_annotations = self.stack_annotations;
        self.stack_annotations = false;
        let argument = self.argument();
        self.stack_annotations = stack_annotations;
        argument
    }

    pub(super) fn display_sum(&mut self) -> Option<Layout> {
        let mut lower = None;
        let mut upper = None;
        loop {
            let remaining = self.remaining.trim_start();
            let limit = match remaining.chars().next() {
                Some('_') => &mut lower,
                Some('^') => &mut upper,
                _ => break,
            };
            if limit.is_some() {
                return None;
            }
            self.remaining = &remaining[1..];
            *limit = Some(self.compact_argument()?);
        }
        // Fix the shared width before centering either limit to avoid cumulative rounding.
        let width = lower
            .iter()
            .chain(&upper)
            .map(Layout::width)
            .max()
            .unwrap_or(1)
            .max(1);
        let mut base = Layout::text(format!("{:^width$}", "∑"));
        if let Some(lower) = lower {
            base = base.annotated(lower, AnnotationPosition::Below)?;
        }
        if let Some(upper) = upper {
            base = base.annotated(upper, AnnotationPosition::Above)?;
        }
        Some(base)
    }

    pub(super) fn structured_command(&mut self, name: &str) -> Option<Layout> {
        match name {
            "begin" => {
                self.remaining = self.remaining.trim_start().strip_prefix("{aligned}")?;
                if self.remaining.trim_start().starts_with('[') {
                    return None;
                }
                self.aligned()
            }
            "boxed" => {
                if !self.display {
                    return None;
                }
                let inner = self.argument()?;
                let width = inner.width();
                if inner.rows.len() + 2 > MAX_ROWS || width + 4 > MAX_COLUMNS {
                    return None;
                }
                let border = "─".repeat(width + 2);
                let mut rows = vec![format!("┌{border}┐")];
                for row in &inner.rows {
                    let padding = " ".repeat(width - display_width(row));
                    rows.push(format!("│ {row}{padding} │"));
                }
                rows.push(format!("└{border}┘"));
                Some(Layout {
                    rows,
                    baseline: inner.baseline + 1,
                })
            }
            "underset" | "overset" => {
                let annotation = self.compact_argument()?;
                let base = self.argument()?;
                if self.stack_annotations {
                    let position = if name == "underset" {
                        AnnotationPosition::Below
                    } else {
                        AnnotationPosition::Above
                    };
                    return base.annotated(annotation, position);
                }
                let annotation = annotation.single()?;
                let base = base.single()?;
                let marker = if name == "underset" { '_' } else { '^' };
                // Parenthesize the base so an annotation cannot attach to only its last atom.
                Some(Layout::text(format!("({base}){marker}{{{annotation}}}")))
            }
            "substack" => {
                self.remaining = self.remaining.trim_start().strip_prefix('{')?;
                let mut rows = Vec::new();
                loop {
                    if rows.len() == MAX_ROWS {
                        return None;
                    }
                    self.remaining = self.remaining.trim_start();
                    let row = self.sequence(SequenceEnd::Cell)?;
                    rows.push(row.single()?.trim().to_owned());
                    if let Some(remaining) = self.remaining.strip_prefix('}') {
                        self.remaining = remaining;
                        break;
                    }
                    self.remaining = self.remaining.strip_prefix(r"\\")?;
                }
                Some(Layout::text(format!("({})", rows.join("; "))))
            }
            "mathcal" => {
                let arg = self.argument()?;
                let text: Option<String> = arg
                    .single()?
                    .chars()
                    .map(|ch| {
                        let index = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".find(ch)?;
                        "𝒜ℬ𝒞𝒟ℰℱ𝒢ℋℐ𝒥𝒦ℒℳ𝒩𝒪𝒫𝒬ℛ𝒮𝒯𝒰𝒱𝒲𝒳𝒴𝒵".chars().nth(index)
                    })
                    .collect();
                Some(Layout::text(text?))
            }
            _ => None,
        }
    }

    fn aligned(&mut self) -> Option<Layout> {
        if !self.display {
            return None;
        }
        let mut rows = Vec::new();
        let mut cells = Vec::new();
        let mut widths = Vec::<usize>::new();
        loop {
            self.remaining = self.remaining.trim_start();
            // A trailing row separator does not introduce an extra blank row.
            if cells.is_empty()
                && let Some(remaining) = self.remaining.strip_prefix(r"\end{aligned}")
            {
                self.remaining = remaining;
                break;
            }
            // Bound empty columns and rows as well as their eventual rendered size.
            if rows.len() == MAX_ROWS || cells.len() == 16 {
                return None;
            }
            let mut cell = self.sequence(SequenceEnd::Cell)?;
            if cell.rows.len() == 1 {
                cell.rows[0] = cell.rows[0].trim().to_owned();
            }
            if widths.len() == cells.len() {
                widths.push(cell.width());
            } else {
                widths[cells.len()] = widths[cells.len()].max(cell.width());
            }
            cells.push(cell);
            if let Some(remaining) = self.remaining.strip_prefix('&') {
                self.remaining = remaining;
                continue;
            }
            if let Some(remaining) = self.remaining.strip_prefix(r"\end{aligned}") {
                self.remaining = remaining;
                rows.push(cells);
                break;
            }
            self.remaining = self.remaining.strip_prefix(r"\\")?.trim_start();
            if let Some(spacing) = self.remaining.strip_prefix('[') {
                let (distance, remaining) = spacing.split_once(']')?;
                let number = [
                    "pt", "pc", "in", "bp", "cm", "mm", "dd", "cc", "sp", "em", "ex",
                ]
                .iter()
                .find_map(|unit| distance.trim().strip_suffix(unit))?;
                if !number.trim().parse::<f64>().ok()?.is_finite() {
                    return None;
                }
                // Physical TeX spacing has no terminal equivalent; retain the row break.
                self.remaining = remaining;
            }
            rows.push(std::mem::take(&mut cells));
        }
        let mut output = Vec::new();
        let mut layout_baseline = 0;
        for cells in rows {
            let baseline = cells.iter().map(|cell| cell.baseline).max()?;
            if output.is_empty() {
                layout_baseline = baseline;
            }
            let height = cells
                .iter()
                .map(|cell| baseline + cell.rows.len() - cell.baseline)
                .max()?;
            if output.len() + height > MAX_ROWS {
                return None;
            }
            for line_index in 0..height {
                let mut line = String::new();
                for (index, cell) in cells.iter().enumerate() {
                    let text = line_index
                        .checked_sub(baseline - cell.baseline)
                        .and_then(|row| cell.rows.get(row))
                        .map(String::as_str)
                        .unwrap_or("");
                    // Align the cell rectangle, preserving numerator/denominator positions.
                    let leading = if index % 2 == 0 {
                        widths[index] - cell.width()
                    } else {
                        0
                    };
                    line.push_str(&" ".repeat(leading));
                    line.push_str(text);
                    line.push_str(&" ".repeat(widths[index] - leading - display_width(text)));
                    if index + 1 < cells.len() {
                        line.push_str(if index % 2 == 0 { " " } else { "  " });
                    }
                }
                if display_width(&line) > MAX_COLUMNS {
                    return None;
                }
                output.push(line.trim_end().to_owned());
            }
        }
        Some(Layout {
            rows: output,
            baseline: layout_baseline,
        })
    }
}
