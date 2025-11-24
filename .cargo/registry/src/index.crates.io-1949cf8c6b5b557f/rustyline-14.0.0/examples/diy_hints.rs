use std::collections::HashSet;

use rustyline::hint::{Hint, Hinter};
use rustyline::history::DefaultHistory;
use rustyline::Context;
use rustyline::{Completer, Helper, Highlighter, Validator};
use rustyline::{Editor, Result};

#[derive(Completer, Helper, Validator, Highlighter)]
struct DIYHinter {
    // It's simple example of rustyline, for more efficient, please use ** radix trie **
    hints: HashSet<CommandHint>,
}

#[derive(Hash, Debug, PartialEq, Eq)]
struct CommandHint {
    display: String,
    complete_up_to: usize,
}

impl Hint for CommandHint {
    fn display(&self) -> &str {
        &self.display
    }

    fn completion(&self) -> Option<&str> {
        if self.complete_up_to > 0 {
            Some(&self.display[..self.complete_up_to])
        } else {
            None
        }
    }
}

impl CommandHint {
    fn new(text: &str, complete_up_to: &str) -> CommandHint {
        assert!(text.starts_with(complete_up_to));
        CommandHint {
            display: text.into(),
            complete_up_to: complete_up_to.len(),
        }
    }

    fn suffix(&self, strip_chars: usize) -> CommandHint {
        CommandHint {
            display: self.display[strip_chars..].to_owned(),
            complete_up_to: self.complete_up_to.saturating_sub(strip_chars),
        }
    }
}

impl Hinter for DIYHinter {
    type Hint = CommandHint;

    fn hint(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Option<CommandHint> {
        if line.is_empty() || pos < line.len() {
            return None;
        }

        self.hints
            .iter()
            .filter_map(|hint| {
                // expect hint after word complete, like redis cli, add condition:
                // line.ends_with(" ")
                if hint.display.starts_with(line) {
                    Some(hint.suffix(pos))
                } else {
                    None
                }
            })
            .next()
    }
}

fn diy_hints() -> HashSet<CommandHint> {
    let mut set = HashSet::new();
    set.insert(CommandHint::new("help", "help"));
    set.insert(CommandHint::new("get key", "get "));
    set.insert(CommandHint::new("set key value", "set "));
    set.insert(CommandHint::new("hget key field", "hget "));
    set.insert(CommandHint::new("hset key field value", "hset "));
    set
}

fn main() -> Result<()> {
    println!("This is a DIY hint hack of rustyline");
    let h = DIYHinter { hints: diy_hints() };

    let mut rl: Editor<DIYHinter, DefaultHistory> = Editor::new()?;
    rl.set_helper(Some(h));

    loop {
        let input = rl.readline("> ")?;
        println!("input: {input}");
    }
}
