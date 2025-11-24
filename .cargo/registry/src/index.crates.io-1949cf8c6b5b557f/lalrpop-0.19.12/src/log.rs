#[derive(Clone)]
pub struct Log {
    level: Level,
}

#[derive(Clone, PartialOrd, Ord, PartialEq, Eq)]
pub enum Level {
    /// No updates unless an error arises.
    Taciturn,

    /// Timing and minimal progress.
    Informative,

    /// More details, but still stuff an end-user is likely to understand.
    Verbose,

    /// Everything you could ever want and then some more.
    Debug,
}

impl Log {
    pub fn new(level: Level) -> Log {
        Log { level }
    }

    pub fn set_level(&mut self, level: Level) {
        self.level = level;
    }

    pub fn log<M>(&self, level: Level, message: M)
    where
        M: FnOnce() -> String,
    {
        if self.level >= level {
            println!("{}", message());
        }
    }
}

macro_rules! log {
    ($session:expr, $level:ident, $($args:expr),*) => {
        $session.log(crate::log::Level::$level, || ::std::fmt::format(format_args!($($args),*)))
    }
}

macro_rules! debug {
    ($($args:expr),*) => {
        log!(crate::tls::Tls::session(), Debug, $($args),*)
    }
}

macro_rules! profile {
    ($session:expr, $phase_name:expr, $action:expr) => {{
        log!($session, Verbose, "Phase `{}` begun", $phase_name);
        let time_stamp = ::std::time::Instant::now();
        let result = $action;
        let elapsed = time_stamp.elapsed();
        log!(
            $session,
            Verbose,
            "Phase `{}` completed in {} seconds",
            $phase_name,
            elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0
        );
        result
    }};
}
