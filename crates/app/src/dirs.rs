//! Cross-platform base directories.
//!
//! `$HOME` exists on macOS/Linux unconditionally, and often on Windows too
//! (Git Bash, WSL-adjacent shells) — when it's set, behavior is exactly what
//! it always was (`~/.local/share/snes-xperience`, `~/.config/snes-xperience`),
//! so nobody's existing data moves. A native Windows launch — an `.exe`
//! double-clicked from Explorer, no shell involved — has no `$HOME`; there,
//! `%APPDATA%\snes-xperience` is the fallback for both, which is the
//! idiomatic single-folder-per-app shape on that platform anyway.

use std::path::PathBuf;

/// Catalog, saves, notebooks.
pub fn data_dir() -> PathBuf {
    base(".local/share")
}

/// `config.toml`.
pub fn config_dir() -> PathBuf {
    base(".config")
}

fn base(unix_suffix: &str) -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(unix_suffix).join("snes-xperience");
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("snes-xperience");
    }
    PathBuf::from(".snes-xperience")
}

#[cfg(test)]
mod tests {
    use super::base;
    use std::path::PathBuf;

    // These touch process-wide env vars, so they run serially by taking a
    // lock — cargo test runs a crate's tests in one process, potentially in
    // parallel, and env vars aren't test-local.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn home_wins_when_set() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_home = std::env::var_os("HOME");
        let prev_appdata = std::env::var_os("APPDATA");
        unsafe {
            std::env::set_var("HOME", "/home/rex");
            std::env::set_var("APPDATA", "C:\\Users\\rex\\AppData\\Roaming");
        }
        assert_eq!(
            base(".local/share"),
            PathBuf::from("/home/rex/.local/share/snes-xperience")
        );
        unsafe {
            restore("HOME", prev_home);
            restore("APPDATA", prev_appdata);
        }
    }

    #[test]
    fn appdata_is_the_windows_fallback_with_no_home() {
        let _g = ENV_LOCK.lock().unwrap();
        let prev_home = std::env::var_os("HOME");
        let prev_appdata = std::env::var_os("APPDATA");
        unsafe {
            std::env::remove_var("HOME");
            std::env::set_var("APPDATA", "C:\\Users\\rex\\AppData\\Roaming");
        }
        assert_eq!(
            base(".config"),
            PathBuf::from("C:\\Users\\rex\\AppData\\Roaming/snes-xperience")
        );
        unsafe {
            restore("HOME", prev_home);
            restore("APPDATA", prev_appdata);
        }
    }

    unsafe fn restore(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}
