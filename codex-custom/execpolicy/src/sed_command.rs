use crate::error::Error;
use crate::error::Result;

pub fn parse_sed_command(sed_command: &str) -> Result<()> {
    // For now, we parse only commands like `122,202p`.
    if let Some(stripped) = sed_command.strip_suffix("p") {
        if let Some((first, rest)) = stripped.split_once(",") {
            if first.parse::<u64>().is_ok() && rest.parse::<u64>().is_ok() {
                return Ok(());
            }
        }
    }

    Err(Error::SedCommandNotProvablySafe {
        command: sed_command.to_string(),
    })
}
