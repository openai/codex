//! Bounded Unicode layout for a deliberately small TeX math subset.
//! Accents apply only to single graphemes so their scope survives terminal rendering.

use crate::width::display_width;
use unicode_segmentation::UnicodeSegmentation;

const MAX_ROWS: usize = 16;
const MAX_COLUMNS: usize = 256;

struct Layout {
    rows: Vec<String>,
    baseline: usize,
}

impl Layout {
    fn text(text: impl Into<String>) -> Self {
        Self {
            rows: vec![text.into()],
            baseline: 0,
        }
    }

    fn width(&self) -> usize {
        self.rows
            .iter()
            .map(|row| display_width(row))
            .max()
            .unwrap_or_default()
    }

    fn join(self, right: Self) -> Option<Self> {
        let baseline = self.baseline.max(right.baseline);
        let height = (baseline + self.rows.len() - self.baseline)
            .max(baseline + right.rows.len() - right.baseline);
        // Separate neighboring fraction bars so their numerators cannot become one number.
        let width = self.width() + usize::from(self.rows.len() > 1 && right.rows.len() > 1);
        if height > MAX_ROWS || width + right.width() > MAX_COLUMNS {
            return None;
        }
        let mut rows = vec![String::new(); height];
        for (index, row) in rows.iter_mut().enumerate() {
            if let Some(left) = index
                .checked_sub(baseline - self.baseline)
                .and_then(|i| self.rows.get(i))
            {
                row.push_str(left);
            }
            row.push_str(&" ".repeat(width - display_width(row)));
            if let Some(right) = index
                .checked_sub(baseline - right.baseline)
                .and_then(|i| right.rows.get(i))
            {
                row.push_str(right);
            }
        }
        Some(Self { rows, baseline })
    }

    fn single(&self) -> Option<&str> {
        (self.rows.len() == 1).then(|| self.rows[0].as_str())
    }
}

pub(super) fn render(source: &str, display: bool) -> Option<String> {
    let mut parser = MathParser {
        remaining: source.trim(),
        depth: 0,
        display,
    };
    let result = parser.sequence(/*group*/ false)?;
    (!result.rows.iter().all(|row| row.trim().is_empty())).then(|| result.rows.join("\n"))
}

struct MathParser<'a> {
    remaining: &'a str,
    depth: usize,
    display: bool,
}

impl MathParser<'_> {
    fn take(&mut self) -> Option<char> {
        let ch = self.remaining.chars().next()?;
        self.remaining = &self.remaining[ch.len_utf8()..];
        Some(ch)
    }

    fn sequence(&mut self, group: bool) -> Option<Layout> {
        if self.depth >= 32 {
            return None;
        }
        self.depth += 1;
        let mut result = Layout::text("");
        let mut scripts = 0;
        let mut has_base = false;
        while let Some(ch) = self.remaining.chars().next() {
            if ch == '}' {
                if !group {
                    return None;
                }
                self.take();
                self.depth -= 1;
                return Some(result);
            }
            if ch == '^' || ch == '_' {
                let script = if ch == '^' { 1 } else { 2 };
                if !has_base || scripts & script != 0 {
                    return None;
                }
                scripts |= script;
            } else if !ch.is_whitespace() {
                scripts = 0;
            }
            let atom = self.atom()?;
            if !ch.is_whitespace() && ch != '^' && ch != '_' {
                // Flattening a compound base would change the scope of a following script.
                has_base = atom
                    .single()
                    .is_some_and(|text| text.graphemes(/*is_extended*/ true).count() == 1);
            }
            result = result.join(atom)?;
        }
        self.depth -= 1;
        (!group).then_some(result)
    }

    fn argument(&mut self) -> Option<Layout> {
        self.remaining = self.remaining.trim_start();
        if self.remaining.starts_with(['^', '_']) {
            return None;
        }
        self.atom()
    }

    fn atom(&mut self) -> Option<Layout> {
        if self.depth >= 32 {
            return None;
        }
        self.depth += 1;
        let result = self.atom_inner();
        self.depth -= 1;
        result
    }

    fn atom_inner(&mut self) -> Option<Layout> {
        let ch = self.take()?;
        match ch {
            '{' => self.sequence(/*group*/ true),
            '\\' => self.command(),
            '^' | '_' => {
                let arg = self.argument()?;
                let (plain, alphabet) = if ch == '^' {
                    (
                        "0123456789+-=()abcdefghijklmnoprstuvwxyz",
                        "⁰¹²³⁴⁵⁶⁷⁸⁹⁺⁻⁼⁽⁾ᵃᵇᶜᵈᵉᶠᵍʰⁱʲᵏˡᵐⁿᵒᵖʳˢᵗᵘᵛʷˣʸᶻ",
                    )
                } else {
                    (
                        "0123456789+-=()aehijklmnoprstuvx",
                        "₀₁₂₃₄₅₆₇₈₉₊₋₌₍₎ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ",
                    )
                };
                let mut output = String::new();
                for value in arg.single()?.chars() {
                    let index = plain.chars().position(|ch| ch == value)?;
                    output.push(alphabet.chars().nth(index)?);
                }
                Some(Layout::text(output))
            }
            '}' | '$' | '%' | '#' | '&' | '`' => None,
            ch if ch.is_whitespace() => {
                self.remaining = self.remaining.trim_start();
                Some(Layout::text(" "))
            }
            ch if ch.is_control() => None,
            ch => Some(Layout::text(ch.to_string())),
        }
    }

    fn command(&mut self) -> Option<Layout> {
        let length = self
            .remaining
            .bytes()
            .take_while(u8::is_ascii_alphabetic)
            .count();
        if length == 0 {
            return match self.take()? {
                ',' | ';' | ':' | ' ' => Some(Layout::text(" ")),
                '!' => Some(Layout::text("")),
                '{' => Some(Layout::text("{")),
                '}' => Some(Layout::text("}")),
                '|' => Some(Layout::text("‖")),
                _ => None,
            };
        }
        let name = &self.remaining[..length];
        self.remaining = &self.remaining[length..];
        match name {
            "frac" | "dfrac" | "tfrac" => {
                let numerator = self.argument()?;
                let denominator = self.argument()?;
                if !self.display {
                    return Some(Layout::text(format!(
                        "(({})/({}))",
                        numerator.single()?,
                        denominator.single()?
                    )));
                }
                // Nested bars need a richer layout to preserve fraction hierarchy.
                let numerator = numerator.single()?;
                let denominator = denominator.single()?;
                let width = display_width(numerator)
                    .max(display_width(denominator))
                    .max(/*other*/ 1);
                if width > MAX_COLUMNS {
                    return None;
                }
                Some(Layout {
                    rows: vec![
                        format!(
                            "{}{numerator}",
                            " ".repeat((width - display_width(numerator)) / 2)
                        ),
                        "─".repeat(width),
                        format!(
                            "{}{denominator}",
                            " ".repeat((width - display_width(denominator)) / 2)
                        ),
                    ],
                    baseline: 1,
                })
            }
            "sqrt" => {
                if self.remaining.trim_start().starts_with('[') {
                    return None;
                }
                let radicand = self.argument()?;
                Some(Layout::text(format!("√({})", radicand.single()?)))
            }
            "mathbb" => {
                let arg = self.argument()?;
                let text = match arg.single()? {
                    "R" => "ℝ",
                    "C" => "ℂ",
                    "N" => "ℕ",
                    "Z" => "ℤ",
                    "Q" => "ℚ",
                    "P" => "ℙ",
                    _ => return None,
                };
                Some(Layout::text(text))
            }
            "hat" | "bar" | "tilde" | "vec" | "dot" | "ddot" => {
                let arg = self.argument()?;
                let text = arg.single()?;
                if text.graphemes(/*is_extended*/ true).count() != 1
                    || text.trim().is_empty()
                    || display_width(text) == 0
                {
                    return None;
                }
                let accent = match name {
                    "hat" => '\u{0302}',
                    "bar" => '\u{0304}',
                    "tilde" => '\u{0303}',
                    "vec" => '\u{20d7}',
                    "dot" => '\u{0307}',
                    "ddot" => '\u{0308}',
                    _ => unreachable!(),
                };
                Some(Layout::text(format!("{text}{accent}")))
            }
            "mathrm" | "mathbf" | "mathit" => self.argument(),
            "text" | "operatorname" => {
                self.remaining = self.remaining.trim_start().strip_prefix('{')?;
                let end = self.remaining.find('}')?;
                let text = &self.remaining[..end];
                if text.contains(['{', '\\', '$', '%', '#', '&'])
                    || text.chars().any(char::is_control)
                {
                    return None;
                }
                self.remaining = &self.remaining[end + 1..];
                Some(Layout::text(text))
            }
            "left" | "right" => {
                self.remaining = self.remaining.trim_start();
                match self.take()? {
                    '.' => Some(Layout::text("")),
                    ch @ ('(' | ')' | '[' | ']' | '|') => Some(Layout::text(ch.to_string())),
                    '<' => Some(Layout::text("⟨")),
                    '>' => Some(Layout::text("⟩")),
                    '\\' => {
                        let length = self
                            .remaining
                            .bytes()
                            .take_while(u8::is_ascii_alphabetic)
                            .count()
                            .max(/*other*/ 1);
                        let (name, remaining) = self.remaining.split_at_checked(length)?;
                        self.remaining = remaining;
                        delimiter(name).map(Layout::text)
                    }
                    _ => None,
                }
            }
            "quad" | "qquad" => Some(Layout::text(" ")),
            "sin" | "cos" | "tan" | "log" | "ln" | "exp" | "lim" | "max" | "min" => {
                Some(Layout::text(name))
            }
            _ => symbol(name).map(Layout::text),
        }
    }
}

fn symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ϵ",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "varkappa" => "ϰ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "varpi" => "ϖ",
        "rho" => "ρ",
        "varrho" => "ϱ",
        "sigma" => "σ",
        "varsigma" => "ς",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "ϕ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "hbar" => "ℏ",
        "ell" => "ℓ",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "aleph" => "ℵ",
        "imath" => "ı",
        "jmath" => "ȷ",
        "prime" => "′",
        "angle" => "∠",
        "dagger" => "†",
        "ddagger" => "‡",
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "cdot" => "·",
        "div" => "÷",
        "circ" => "∘",
        "bullet" => "∙",
        "oplus" => "⊕",
        "otimes" => "⊗",
        "odot" => "⊙",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "propto" => "∝",
        "sim" => "∼",
        "simeq" => "≃",
        "cong" => "≅",
        "lesssim" => "≲",
        "gtrsim" => "≳",
        "ll" => "≪",
        "gg" => "≫",
        "perp" => "⊥",
        "parallel" => "∥",
        "equiv" => "≡",
        "in" => "∈",
        "notin" => "∉", // codespell:ignore notin
        "ni" => "∋",
        "subset" => "⊂",
        "subseteq" => "⊆",
        "supset" => "⊃",
        "supseteq" => "⊇",
        "cup" => "∪",
        "cap" => "∩",
        "bigcup" => "⋃",
        "bigcap" => "⋂",
        "setminus" => "∖",
        "emptyset" | "varnothing" => "∅",
        "land" | "wedge" => "∧",
        "lor" | "vee" => "∨",
        "neg" | "lnot" => "¬",
        "top" => "⊤",
        "bot" => "⊥",
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "to" | "rightarrow" => "→",
        "leftarrow" => "←",
        "leftrightarrow" => "↔",
        "mapsto" => "↦",
        "uparrow" => "↑",
        "downarrow" => "↓",
        "updownarrow" => "↕",
        "Leftarrow" | "impliedby" => "⇐",
        "Rightarrow" | "implies" => "⇒",
        "Leftrightarrow" | "iff" => "⇔",
        "ldots" | "dots" => "…",
        "cdots" => "⋯",
        "vdots" => "⋮",
        "ddots" => "⋱",
        _ => return delimiter(name),
    })
}

fn delimiter(name: &str) -> Option<&'static str> {
    Some(match name {
        "langle" => "⟨",
        "rangle" => "⟩",
        "lbrace" | "{" => "{",
        "rbrace" | "}" => "}",
        "lbrack" => "[",
        "rbrack" => "]",
        "vert" | "lvert" | "rvert" => "|",
        "Vert" | "lVert" | "rVert" | "|" => "‖",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "lceil" => "⌈",
        "rceil" => "⌉",
        _ => return None,
    })
}
