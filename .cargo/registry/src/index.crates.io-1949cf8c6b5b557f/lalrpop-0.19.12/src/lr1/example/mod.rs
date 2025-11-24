//! Code to compute example inputs given a backtrace.

use ascii_canvas::AsciiView;
use crate::grammar::repr::*;
use crate::message::builder::InlineBuilder;
use crate::message::Content;
use std::fmt::{Debug, Error, Formatter};
use crate::style::Style;
use crate::tls::Tls;

#[cfg(test)]
mod test;

/// An "example" input and the way it was derived. This can be
/// serialized into useful text. For example, it might represent
/// something like this:
///
/// ```
///          Looking at
///              |
///              v
/// Ty "->" Ty "->" Ty
/// |        |       |
/// +-Ty-----+       |
/// |                |
/// +-Ty-------------+
/// ```
///
/// The top-line is the `symbols` vector. The groupings below are
/// stored in the `reductions` vector, in order from smallest to
/// largest (they are always properly nested). The `cursor` field
/// indicates the current lookahead token.
///
/// The `symbols` vector is actually `Option<Symbol>` to account
/// for empty reductions:
///
/// ```
/// A       B
/// | |   | |
/// | +-Y-+ |
/// +-Z-----+
/// ```
///
/// The "empty space" between A and B would be represented as `None`.
#[derive(Clone, Debug)]
pub struct Example {
    pub symbols: Vec<ExampleSymbol>,
    pub cursor: usize,
    pub reductions: Vec<Reduction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExampleSymbol {
    Symbol(Symbol),
    Epsilon,
}

#[derive(Copy, Clone, Default)]
pub struct ExampleStyles {
    pub before_cursor: Style,
    pub on_cursor: Style,
    pub after_cursor: Style,
}

#[derive(Clone, Debug)]
pub struct Reduction {
    pub start: usize,
    pub end: usize,
    pub nonterminal: NonterminalString,
}

impl Example {
    /// Length of each symbol. Each will need *at least* that amount
    /// of space. :) Measure in characters, under the assumption of a
    /// mono-spaced font. Also add a final `0` marker which will serve
    /// as the end position.
    fn lengths(&self) -> Vec<usize> {
        self.symbols
            .iter()
            .map(|s| match *s {
                ExampleSymbol::Symbol(ref s) => format!("{}", s).chars().count(),
                ExampleSymbol::Epsilon => 1, // display as " "
            })
            .chain(Some(0))
            .collect()
    }

    /// Extract a prefix of the list of symbols from this `Example`
    /// and make a styled list of them, like:
    ///
    ///    Ty "->" Ty -> "Ty"
    pub fn to_symbol_list(&self, length: usize, styles: ExampleStyles) -> Box<dyn Content> {
        let mut builder = InlineBuilder::new().begin_spaced();

        for (index, symbol) in self.symbols[..length].iter().enumerate() {
            let style = if index < self.cursor {
                styles.before_cursor
            } else if index > self.cursor {
                styles.after_cursor
            } else {
                match *symbol {
                    ExampleSymbol::Symbol(Symbol::Terminal(_)) => styles.on_cursor,
                    ExampleSymbol::Symbol(Symbol::Nonterminal(_)) => styles.after_cursor,
                    ExampleSymbol::Epsilon => styles.after_cursor,
                }
            };

            if let ExampleSymbol::Symbol(ref s) = symbol {
                builder = builder.push(s.clone()).styled(style);
            }
        }

        builder.end().indented().end()
    }

    /// Render the example into a styled diagram suitable for
    /// embedding in an error message.
    pub fn into_picture(self, styles: ExampleStyles) -> Box<dyn Content> {
        let lengths = self.lengths();
        let positions = self.positions(&lengths);
        InlineBuilder::new()
            .push(Box::new(ExamplePicture {
                example: self,
                positions,
                styles,
            }))
            .indented()
            .end()
    }

    fn starting_positions(&self, lengths: &[usize]) -> Vec<usize> {
        lengths
            .iter()
            .scan(0, |counter, &len| {
                let start = *counter;

                // Leave space for "NT " (if "NT" is the name
                // of the nonterminal).
                *counter = start + len + 1;

                Some(start)
            })
            .collect()
    }

    /// Start index where each symbol in the example should appear,
    /// measured in characters. These are spaced to leave enough room
    /// for the reductions below.
    fn positions(&self, lengths: &[usize]) -> Vec<usize> {
        // Initially, position each symbol with one space in between,
        // like:
        //
        //     X Y Z
        let mut positions = self.starting_positions(lengths);

        // Adjust spacing to account for the nonterminal labels
        // we will have to add. It will display
        // like this:
        //
        //    A1 B2 C3 D4 E5 F6
        //    |         |
        //    +-Label---+
        //
        // But if the label is long we may have to adjust the spacing
        // of the covered items (here, we changed them to two spaces,
        // except the first gap, which got 3 spaces):
        //
        //    A1   B2  C3  D4 E5 F6
        //    |             |
        //    +-LongLabel22-+
        for &Reduction {
            start,
            end,
            ref nonterminal,
        } in &self.reductions
        {
            let nt_len = format!("{}", nonterminal).chars().count();

            // Number of symbols we are reducing. This should always
            // be non-zero because even in the case of a \epsilon
            // rule, we ought to be have a `None` entry in the symbol array.
            let num_syms = end - start;
            assert!(num_syms > 0);

            // Let's use the expansion from above as our running example.
            // We start out with positions like this:
            //
            //    A1 B2 C3 D4 E5 F6
            //    |             |
            //    +-LongLabel22-+
            //
            // But we want LongLabel to end at D4. No good.

            // Start of first symbol to be reduced. Here, 0.
            //
            // A1 B2 C3 D4
            // ^ here
            let start_position = positions[start];

            // End of last symbol to be reduced. Here, 11.
            //
            // A1 B2 C3 D4 E5
            //             ^ positions[end]
            //            ^ here -- positions[end] - 1
            let end_position = positions[end] - 1;

            // We need space to draw `+-Label-+` between
            // start_position and end_position.
            let required_len = nt_len + 4; // here, 15
            let actual_len = end_position - start_position; // here, 10
            if required_len < actual_len {
                continue; // Got enough space, all set.
            }

            // Have to add `difference` characters altogether.
            let difference = required_len - actual_len; // here, 4

            // Increment over everything that is not part of this nonterminal.
            // In the example above, that is E5 and F6.
            shift(&mut positions[end..], difference);

            if num_syms > 1 {
                // If there is just one symbol being reduced here,
                // then we have shifted over the things that follow
                // it, and we are done. This would be a case like:
                //
                //     X         Y Z
                //     |       |
                //     +-Label-+
                //
                // (which maybe ought to be rendered slightly
                // differently).
                //
                // But if there are multiple symbols, we're not quite
                // done, because there would be an unsightly gap:
                //
                //       (gaps)
                //      |  |  |
                //      v  v  v
                //    A1 B2 C3 D4     E5 F6
                //    |             |
                //    +-LongLabel22-+
                //
                // we'd like to make things line up, so we have to
                // distribute that extra space internally by
                // increasing the "gaps" (marked above) as evenly as
                // possible (basically, full justification).
                //
                // We do this by dividing up the spaces evenly and
                // then taking the remainder `N` and distributing 1
                // extra to the first N.
                let num_gaps = num_syms - 1; // number of gaps we can adjust. Here, 3.
                let amount = difference / num_gaps; // what to add to each gap. Here, 1.
                let extra = difference % num_gaps; // the remainder. Here, 1.

                // For the first `extra` symbols, give them amount + 1
                // extra space. After that, just amount. (O(n^2). Sue me.)
                for i in 0..extra {
                    shift(&mut positions[start + 1 + i..end], amount + 1);
                }
                for i in extra..num_gaps {
                    shift(&mut positions[start + 1 + i..end], amount);
                }
            }
        }

        positions
    }

    #[cfg(test)]
    pub fn paint_unstyled(&self) -> Vec<::ascii_canvas::Row> {
        let this = self.clone();
        let content = this.into_picture(ExampleStyles::default());
        let min_width = content.min_width();
        let canvas = content.emit_to_canvas(min_width);
        canvas.to_strings()
    }

    fn paint_on(&self, styles: &ExampleStyles, positions: &[usize], view: &mut dyn AsciiView) {
        // Draw the brackets for each reduction:
        for (index, reduction) in self.reductions.iter().enumerate() {
            let start_column = positions[reduction.start];
            let end_column = positions[reduction.end] - 1;
            let row = 1 + index;
            view.draw_vertical_line(0..row + 1, start_column);
            view.draw_vertical_line(0..row + 1, end_column - 1);
            view.draw_horizontal_line(row, start_column..end_column);
        }

        // Write the labels for each reduction. Do this after the
        // brackets so that ascii canvas can convert `|` to `+`
        // without interfering with the text (in case of weird overlap).
        let session = Tls::session();
        for (index, reduction) in self.reductions.iter().enumerate() {
            let column = positions[reduction.start] + 2;
            let row = 1 + index;
            view.write_chars(
                row,
                column,
                reduction.nonterminal.to_string().chars(),
                session.nonterminal_symbol,
            );
        }

        // Write the labels on top:
        //    A1   B2  C3  D4 E5 F6
        self.paint_symbols_on(&self.symbols, positions, styles, view);
    }

    fn paint_symbols_on(
        &self,
        symbols: &[ExampleSymbol],
        positions: &[usize],
        styles: &ExampleStyles,
        view: &mut dyn AsciiView,
    ) {
        let session = Tls::session();
        for (index, ex_symbol) in symbols.iter().enumerate() {
            let style = if index < self.cursor {
                styles.before_cursor
            } else if index == self.cursor {
                // Only display actual terminals in the "on-cursor"
                // font, because it might be misleading to show a
                // nonterminal that way. Really it'd be nice to expand
                // so that the cursor is always a terminal.
                match *ex_symbol {
                    ExampleSymbol::Symbol(Symbol::Terminal(_)) => styles.on_cursor,
                    _ => styles.after_cursor,
                }
            } else {
                styles.after_cursor
            };

            let column = positions[index];
            match *ex_symbol {
                ExampleSymbol::Symbol(Symbol::Terminal(ref term)) => {
                    view.write_chars(
                        0,
                        column,
                        term.to_string().chars(),
                        style.with(session.terminal_symbol),
                    );
                }
                ExampleSymbol::Symbol(Symbol::Nonterminal(ref nt)) => {
                    view.write_chars(
                        0,
                        column,
                        nt.to_string().chars(),
                        style.with(session.nonterminal_symbol),
                    );
                }
                ExampleSymbol::Epsilon => {}
            }
        }
    }
}

struct ExamplePicture {
    example: Example,
    positions: Vec<usize>,
    styles: ExampleStyles,
}

impl Content for ExamplePicture {
    fn min_width(&self) -> usize {
        *self.positions.last().unwrap()
    }

    fn emit(&self, view: &mut dyn AsciiView) {
        self.example.paint_on(&self.styles, &self.positions, view);
    }

    fn into_wrap_items(self: Box<Self>, wrap_items: &mut Vec<Box<dyn Content>>) {
        wrap_items.push(self);
    }
}

impl Debug for ExamplePicture {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        Debug::fmt(&self.example, fmt)
    }
}

fn shift(positions: &mut [usize], amount: usize) {
    for position in positions {
        *position += amount;
    }
}

impl ExampleStyles {
    pub fn ambig() -> Self {
        let session = Tls::session();
        ExampleStyles {
            before_cursor: session.ambig_symbols,
            on_cursor: session.ambig_symbols,
            after_cursor: session.ambig_symbols,
        }
    }

    pub fn new() -> Self {
        let session = Tls::session();
        ExampleStyles {
            before_cursor: session.observed_symbols,
            on_cursor: session.cursor_symbol,
            after_cursor: session.unobserved_symbols,
        }
    }
}
