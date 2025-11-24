use std::process::Command;

use log::error;

pub fn get() -> Option<String> {
    Command::new("uname")
        .arg("-m")
        .output()
        .map_err(|e| {
            error!("Cannot invoke 'uname` to get architecture type: {:?}", e);
        })
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim_end().to_owned())
            } else {
                log::error!("'uname' invocation error: {:?}", out);
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uname_nonempty() {
        let val = get().expect("architecture::get() failed");
        assert!(!val.is_empty());
    }
}
