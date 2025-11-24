use std::process::Command;

use log::error;

pub fn uname(arg: &str) -> Option<String> {
    Command::new("uname")
        .arg(arg)
        .output()
        .map_err(|e| {
            error!("Failed to invoke 'uname {}': {:?}", arg, e);
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
        let val = uname("-s").expect("uname failed");
        assert!(!val.is_empty());
    }
}
