/*!
Specify logging filters in code instead of using an environment variable.
*/

use env_logger::Builder;

use log::{debug, error, info, trace, warn, LevelFilter};

fn main() {
    Builder::new().filter_level(LevelFilter::max()).init();

    trace!("some trace log");
    debug!("some debug log");
    info!("some information log");
    warn!("some warning log");
    error!("some error log");
}
