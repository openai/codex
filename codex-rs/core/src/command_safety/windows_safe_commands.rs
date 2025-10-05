// This is a WIP. This will eventually contain a real list of common safe Windows commands.
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

// Track which commands have been "always allowed" by the user
static mut ALWAYS_ALLOWED_COMMANDS: Option<HashSet<Vec<String>>> = None;
static mut ALWAYS_ALLOWED_FILE: Option<PathBuf> = None;

pub fn is_safe_command_windows(command: &[String]) -> bool {
    load_always_allowed();
    // Check if this command has been always allowed by the user
    unsafe {
        let ptr = std::ptr::addr_of!(ALWAYS_ALLOWED_COMMANDS);
        if let Some(ref allowed) = *ptr {
            if allowed.contains(&command.to_vec()) {
                return true;
            }
        }
    }
    
    // Basic PowerShell commands that are generally safe
    if command.len() >= 2 && command[0] == "powershell.exe" {
        // Check for common safe PowerShell commands
        if let Some(second) = command.get(1) {
            match second.as_str() {
                // Safe commands with basic operations
                "-NoLogo" | "-Command" => {
                    // Check if the actual command is safe
                    if command.len() >= 4 && command[3] == "echo" {
                        return true;
                    }
                    if command.len() >= 4 && command[3] == "Get-ChildItem" {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    
    // Basic file operations that are generally safe
    if command.len() >= 1 {
        return match command[0].as_str() {
            "echo" | "dir" | "type" | "cls" => true,
            _ => false,
        };
    }

    false
}

pub fn mark_command_always_allowed(command: &[String]) {
    load_always_allowed();
    unsafe {
        let ptr = std::ptr::addr_of_mut!(ALWAYS_ALLOWED_COMMANDS);
        if let Some(ref mut allowed) = *ptr {
            allowed.insert(command.to_vec());
        }
    }
    save_always_allowed();
}

pub fn set_always_allowed_file(path: PathBuf) {
    unsafe {
        ALWAYS_ALLOWED_FILE = Some(path);
    }
}

fn load_always_allowed() {
    unsafe {
        let ptr = std::ptr::addr_of!(ALWAYS_ALLOWED_COMMANDS);
        if (*ptr).is_none() {
            let mut value = None;
            if let Some(ref file) = ALWAYS_ALLOWED_FILE {
                if file.exists() {
                    if let Ok(content) = fs::read_to_string(file) {
                        if let Ok(allowed) = serde_json::from_str::<HashSet<Vec<String>>>(&content) {
                            value = Some(allowed);
                        }
                    }
                }
            }
            if value.is_none() {
                value = Some(HashSet::new());
            }
            ALWAYS_ALLOWED_COMMANDS = value;
        }
    }
}

fn save_always_allowed() {
    unsafe {
        if let Some(ref file) = ALWAYS_ALLOWED_FILE {
            let ptr = std::ptr::addr_of!(ALWAYS_ALLOWED_COMMANDS);
            if let Some(ref allowed) = *ptr {
                if let Ok(content) = serde_json::to_string(allowed) {
                    let _ = fs::write(file, content);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_safe_command_windows;

    fn vec_str(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn everything_is_unsafe() {
        for cmd in [
            vec_str(&["powershell.exe", "-NoLogo", "-Command", "echo hello"]),
            vec_str(&["copy", "foo", "bar"]),
            vec_str(&["del", "file.txt"]),
            vec_str(&["powershell.exe", "Get-ChildItem"]),
        ] {
            assert!(!is_safe_command_windows(&cmd));
        }
    }

    #[test]
    fn safe_powershell_commands() {
        // These should be considered safe
        let safe_commands = [
            vec_str(&["powershell.exe", "-NoLogo", "-Command", "echo", "hello"])
        ];
        for cmd in safe_commands {
            assert!(is_safe_command_windows(&cmd));
        }
    }
}
