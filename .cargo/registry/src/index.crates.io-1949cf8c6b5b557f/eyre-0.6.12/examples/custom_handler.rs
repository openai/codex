use backtrace::Backtrace;
use eyre::EyreHandler;
use std::error::Error;
use std::{fmt, iter};

fn main() -> eyre::Result<()> {
    // Install our custom eyre report hook for constructing our custom Handlers
    install().unwrap();

    // construct a report with, hopefully, our custom handler!
    let mut report = eyre::eyre!("hello from custom error town!");

    // manually set the custom msg for this report after it has been constructed
    if let Some(handler) = report.handler_mut().downcast_mut::<Handler>() {
        handler.custom_msg = Some("you're the best users, you know that right???");
    }

    // print that shit!!
    Err(report)
}

// define a handler that captures backtraces unless told not to
fn install() -> Result<(), impl Error> {
    let capture_backtrace = std::env::var("RUST_BACKWARDS_TRACE")
        .map(|val| val != "0")
        .unwrap_or(true);

    let hook = Hook { capture_backtrace };

    eyre::set_hook(Box::new(move |e| Box::new(hook.make_handler(e))))
}

struct Hook {
    capture_backtrace: bool,
}

impl Hook {
    fn make_handler(&self, _error: &(dyn Error + 'static)) -> Handler {
        let backtrace = if self.capture_backtrace {
            Some(Backtrace::new())
        } else {
            None
        };

        Handler {
            backtrace,
            custom_msg: None,
        }
    }
}

struct Handler {
    // custom configured backtrace capture
    backtrace: Option<Backtrace>,
    // customizable message payload associated with reports
    custom_msg: Option<&'static str>,
}

impl EyreHandler for Handler {
    fn debug(&self, error: &(dyn Error + 'static), f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            return fmt::Debug::fmt(error, f);
        }

        let errors = iter::successors(Some(error), |error| (*error).source());

        for (ind, error) in errors.enumerate() {
            write!(f, "\n{:>4}: {}", ind, error)?;
        }

        if let Some(backtrace) = self.backtrace.as_ref() {
            writeln!(f, "\n\nBacktrace:\n{:?}", backtrace)?;
        }

        if let Some(msg) = self.custom_msg.as_ref() {
            writeln!(f, "\n\n{}", msg)?;
        }

        Ok(())
    }
}
