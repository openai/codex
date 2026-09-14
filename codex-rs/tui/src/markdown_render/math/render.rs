//! Bounded Unicode layout for a deliberately small TeX math subset.

use crate::width::display_width;

pub(super) fn render(source: &str) -> Option<String> {
    let mut parser = MathParser {
        remaining: source.trim(),
        depth: 0,
    };
    let result = parser.sequence(/*group*/ false)?;
    (!result.trim().is_empty()).then_some(result)
}

struct MathParser<'a> {
    remaining: &'a str,
    depth: usize,
}

impl MathParser<'_> {
    fn take(&mut self) -> Option<char> {
        let ch = self.remaining.chars().next()?;
        self.remaining = &self.remaining[ch.len_utf8()..];
        Some(ch)
    }

    fn sequence(&mut self, group: bool) -> Option<String> {
        if self.depth >= 32 {
            return None;
        }
        self.depth += 1;
        let mut result = String::new();
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
                has_base = atom.chars().count() == 1;
            }
            result.push_str(&atom);
            if display_width(&result) > 256 {
                return None;
            }
        }
        self.depth -= 1;
        (!group).then_some(result)
    }

    fn argument(&mut self) -> Option<String> {
        self.remaining = self.remaining.trim_start();
        if self.remaining.starts_with(['^', '_']) {
            return None;
        }
        self.atom()
    }

    fn atom(&mut self) -> Option<String> {
        if self.depth >= 32 {
            return None;
        }
        self.depth += 1;
        let result = self.atom_inner();
        self.depth -= 1;
        result
    }

    fn atom_inner(&mut self) -> Option<String> {
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
                for value in arg.chars() {
                    let index = plain.chars().position(|ch| ch == value)?;
                    output.push(alphabet.chars().nth(index)?);
                }
                Some(output)
            }
            '}' | '$' | '%' | '#' | '&' | '`' => None,
            ch if ch.is_whitespace() => {
                self.remaining = self.remaining.trim_start();
                Some(String::from(" "))
            }
            ch if ch.is_control() => None,
            ch => Some(ch.to_string()),
        }
    }

    fn command(&mut self) -> Option<String> {
        let length = self
            .remaining
            .bytes()
            .take_while(u8::is_ascii_alphabetic)
            .count();
        if length == 0 {
            return match self.take()? {
                ',' | ';' | ':' | ' ' => Some(String::from(" ")),
                '!' => Some(String::new()),
                '{' => Some(String::from("{")),
                '}' => Some(String::from("}")),
                '|' => Some(String::from("‖")),
                _ => None,
            };
        }
        let name = &self.remaining[..length];
        self.remaining = &self.remaining[length..];
        match name {
            "frac" | "dfrac" | "tfrac" => {
                let numerator = self.argument()?;
                let denominator = self.argument()?;
                Some(format!("(({numerator})/({denominator}))"))
            }
            "sqrt" => {
                if self.remaining.trim_start().starts_with('[') {
                    return None;
                }
                let radicand = self.argument()?;
                Some(format!("√({radicand})"))
            }
            "mathbb" => {
                let arg = self.argument()?;
                let text = match arg.as_str() {
                    "R" => "ℝ",
                    "C" => "ℂ",
                    "N" => "ℕ",
                    "Z" => "ℤ",
                    "Q" => "ℚ",
                    "P" => "ℙ",
                    _ => return None,
                };
                Some(String::from(text))
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
                Some(String::from(text))
            }
            "left" | "right" => {
                self.remaining = self.remaining.trim_start();
                match self.take()? {
                    '.' => Some(String::new()),
                    ch @ ('(' | ')' | '[' | ']' | '|') => Some(ch.to_string()),
                    _ => None,
                }
            }
            "quad" | "qquad" => Some(String::from(" ")),
            "sin" | "cos" | "tan" | "log" | "ln" | "exp" | "lim" | "max" | "min" => {
                Some(String::from(name))
            }
            _ => symbol(name).map(String::from),
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
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "pi" => "π",
        "rho" => "ρ",
        "sigma" => "σ",
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
        "int" => "∫",
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "pm" => "±",
        "mp" => "∓",
        "times" => "×",
        "cdot" => "·",
        "div" => "÷",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "equiv" => "≡",
        "in" => "∈",
        "notin" => "∉", // codespell:ignore notin
        "subset" => "⊂",
        "subseteq" => "⊆",
        "cup" => "∪",
        "cap" => "∩",
        "emptyset" => "∅",
        "forall" => "∀",
        "exists" => "∃",
        "to" | "rightarrow" => "→",
        "leftarrow" => "←",
        "Rightarrow" => "⇒",
        "Leftrightarrow" => "⇔",
        "ldots" | "dots" => "…",
        "cdots" => "⋯",
        _ => return None,
    })
}
