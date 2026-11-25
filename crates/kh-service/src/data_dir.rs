//! Where the app keeps its per-user files, for programs that start without the window.

use std::ffi::OsString;
use std::path::PathBuf;

/// The app identifier. It must match `identifier` in the shell's `tauri.conf.json`.
pub const APP_ID: &str = "co.kolaborate.kolahub";
/// Overrides the folder, for tests and portable setups.
pub const DATA_DIR_VARIABLE: &str = "KH_DATA_DIR";

/// The folder the window uses on this system, or `None` when the system names no home folder.
pub fn default_data_dir() -> Option<PathBuf> {
    resolve(|name| std::env::var_os(name).filter(|v| !v.is_empty()))
}

fn resolve(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(chosen) = var(DATA_DIR_VARIABLE) {
        return Some(PathBuf::from(chosen));
    }
    let base = if cfg!(windows) {
        PathBuf::from(var("APPDATA")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(var("HOME")?).join("Library/Application Support")
    } else {
        match var("XDG_DATA_HOME") {
            Some(data) => PathBuf::from(data),
            None => PathBuf::from(var("HOME")?).join(".local/share"),
        }
    };
    Some(base.join(APP_ID))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_override_wins_and_the_default_ends_in_the_app_id() {
        let chosen = resolve(|n| (n == DATA_DIR_VARIABLE).then(|| "chosen".into()));
        assert_eq!(chosen, Some(PathBuf::from("chosen")));
        let usual = resolve(|n| (n != DATA_DIR_VARIABLE).then(|| "base".into())).expect("folder");
        assert!(usual.ends_with(APP_ID));
        assert_eq!(resolve(|_| None), None);
    }

    #[test]
    fn the_app_id_matches_the_window_configuration() {
        let config = include_str!("../../kh-app/tauri.conf.json");
        assert!(config.contains(&format!("\"identifier\": \"{APP_ID}\"")));
    }
}
