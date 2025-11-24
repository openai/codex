use libc::c_int;
use signal_hook::consts::signal::*;
use signal_hook::low_level;

use std::io::Error;

#[cfg(feature = "extended-siginfo")]
type Signals =
    signal_hook::iterator::SignalsInfo<signal_hook::iterator::exfiltrator::origin::WithOrigin>;

#[cfg(not(feature = "extended-siginfo"))]
use signal_hook::iterator::Signals;

fn main() -> Result<(), Error> {
    const SIGNALS: &[c_int] = &[
        SIGTERM, SIGQUIT, SIGINT, SIGTSTP, SIGWINCH, SIGHUP, SIGCHLD, SIGCONT,
    ];
    let mut sigs = Signals::new(SIGNALS)?;
    for signal in &mut sigs {
        eprintln!("Received signal {:?}", signal);
        #[cfg(feature = "extended-siginfo")]
        let signal = signal.signal;
        // After printing it, do whatever the signal was supposed to do in the first place
        low_level::emulate_default_handler(signal)?;
    }
    Ok(())
}
