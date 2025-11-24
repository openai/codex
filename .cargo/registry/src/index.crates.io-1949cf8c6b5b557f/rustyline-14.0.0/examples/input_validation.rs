use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Completer, Helper, Highlighter, Hinter};
use rustyline::{Editor, Result};

#[derive(Completer, Helper, Highlighter, Hinter)]
struct InputValidator {}

impl Validator for InputValidator {
    fn validate(&self, ctx: &mut ValidationContext) -> Result<ValidationResult> {
        use ValidationResult::{Incomplete, Invalid, Valid};
        let input = ctx.input();
        let result = if !input.starts_with("SELECT") {
            Invalid(Some(" --< Expect: SELECT stmt".to_owned()))
        } else if !input.ends_with(';') {
            Incomplete
        } else {
            Valid(None)
        };
        Ok(result)
    }
}

fn main() -> Result<()> {
    let h = InputValidator {};
    let mut rl = Editor::new()?;
    rl.set_helper(Some(h));

    let input = rl.readline("> ")?;
    println!("Input: {input}");
    Ok(())
}
