use log::{LevelFilter, Log, Metadata, Record};

#[allow(dead_code)]
struct Logger {
    pub filter: Option<String>,
}

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        eprintln!(
            "[{}] {}",
            record
                .module_path()
                .unwrap_or_default()
                .trim_start_matches("rust_tree_sitter_cli::"),
            record.args()
        );
    }

    fn flush(&self) {}
}

pub fn init() {
    log::set_boxed_logger(Box::new(Logger { filter: None })).unwrap();
    log::set_max_level(LevelFilter::Info);
}
