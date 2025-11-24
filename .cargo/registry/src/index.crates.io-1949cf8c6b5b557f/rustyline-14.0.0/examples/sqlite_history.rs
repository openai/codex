use rustyline::{Config, Editor, Result};

fn main() -> Result<()> {
    let config = Config::builder().auto_add_history(true).build();
    let history = if false {
        // memory
        rustyline::sqlite_history::SQLiteHistory::with_config(config)?
    } else {
        // file
        rustyline::sqlite_history::SQLiteHistory::open(config, "history.sqlite3")?
    };
    let mut rl: Editor<(), _> = Editor::with_history(config, history)?;
    loop {
        let line = rl.readline("> ")?;
        println!("{line}");
    }
}
