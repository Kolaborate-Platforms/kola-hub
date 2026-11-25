//! Settings, language strings, shortcuts and brand tokens.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

/// The English catalogue as written, for the interface to fill placeholders by the same rules.
pub const ENGLISH_JSON: &str = include_str!("../strings/en.json");
/// A settings file is a few hundred bytes. Anything far larger is not ours.
const MAX_SETTINGS_BYTES: u64 = 64 * 1024;

/// Brand colours used by the app. Values come from spec section 8.
pub const BRAND: BrandTokens = BrandTokens {
    primary: "#35b544",
    secondary: "#e1f4e4",
    tertiary: "#2a8a34",
    grey: "#424242",
    yellow: "#fbdf64",
    font: "Poppins",
};

/// Shared visual tokens, also available as CSS custom properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrandTokens {
    /// Primary green.
    pub primary: &'static str,
    /// Pale green.
    pub secondary: &'static str,
    /// Dark green.
    pub tertiary: &'static str,
    /// Grey.
    pub grey: &'static str,
    /// Yellow.
    pub yellow: &'static str,
    /// Brand font family.
    pub font: &'static str,
}

impl BrandTokens {
    /// Return the brand values as CSS custom properties.
    pub fn to_css(self) -> String {
        format!(
            ":root {{ --kh-primary: {}; --kh-secondary: {}; --kh-tertiary: {}; --kh-grey: {}; --kh-yellow: {}; --kh-font: '{}'; }}",
            self.primary, self.secondary, self.tertiary, self.grey, self.yellow, self.font
        )
    }
}

/// A localized message, either plain text or a one/other plural form.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Message {
    /// A message without plural forms.
    Text(String),
    /// English singular and plural forms.
    Plural(BTreeMap<String, String>),
}

/// English catalogue loaded from the bundled JSON file.
pub fn english_catalogue() -> Result<BTreeMap<String, Message>, serde_json::Error> {
    serde_json::from_str(ENGLISH_JSON)
}

fn cached_english() -> Option<&'static BTreeMap<String, Message>> {
    static CATALOGUE: OnceLock<Option<BTreeMap<String, Message>>> = OnceLock::new();
    CATALOGUE.get_or_init(|| english_catalogue().ok()).as_ref()
}

/// Look up a message, select its English plural form, and replace named placeholders.
/// Unknown keys are returned unchanged so the interface can show the key.
pub fn translate(key: &str, count: Option<u64>, values: &BTreeMap<String, String>) -> String {
    let Some(catalogue) = cached_english() else {
        return key.to_owned();
    };
    let Some(message) = catalogue.get(key) else {
        return key.to_owned();
    };
    let text = match message {
        Message::Text(text) => text.as_str(),
        Message::Plural(forms) => {
            let form = if count == Some(1) { "one" } else { "other" };
            let Some(text) = forms.get(form) else {
                return key.to_owned();
            };
            text.as_str()
        }
    };
    fill(text, count, values)
}

/// Replaces placeholders in one pass, so a value that contains braces is never filled again.
fn fill(text: &str, count: Option<u64>, values: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, after)) = rest.split_once('{') {
        out.push_str(before);
        let Some((name, tail)) = after.split_once('}') else {
            out.push('{');
            rest = after;
            continue;
        };
        match (values.get(name), count) {
            (Some(value), _) => out.push_str(value),
            (None, Some(count)) if name == "count" => out.push_str(&count.to_string()),
            _ => out.push_str(&format!("{{{name}}}")),
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// Check that keys, plural forms, and `{placeholder}` names are well formed.
pub fn validate_catalogue(catalogue: &BTreeMap<String, Message>) -> Result<(), String> {
    for (key, message) in catalogue {
        if key.trim().is_empty() {
            return Err("catalogue keys cannot be empty".into());
        }
        match message {
            Message::Text(text) => validate_placeholders(text)?,
            Message::Plural(forms) => {
                if forms.len() != 2 || !forms.contains_key("one") || !forms.contains_key("other") {
                    return Err(format!("{key}: plural must contain exactly one and other"));
                }
                for text in forms.values() {
                    if text.trim().is_empty() {
                        return Err(format!("{key}: plural text cannot be empty"));
                    }
                    validate_placeholders(text)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_placeholders(text: &str) -> Result<(), String> {
    let mut opening = false;
    let mut name = String::new();
    for ch in text.chars() {
        match ch {
            '{' if !opening => {
                opening = true;
                name.clear();
            }
            '}' if opening && !name.is_empty() => opening = false,
            '}' => return Err(format!("malformed placeholder in {text:?}")),
            '{' => return Err(format!("nested placeholder in {text:?}")),
            _ if opening && (ch.is_alphanumeric() || ch == '_') => name.push(ch),
            _ if opening => return Err(format!("invalid placeholder in {text:?}")),
            _ => {}
        }
    }
    if opening {
        return Err(format!("unclosed placeholder in {text:?}"));
    }
    Ok(())
}

/// Relative luminance contrast ratio for two `#RRGGBB` colours.
pub fn contrast_ratio(foreground: &str, background: &str) -> Result<f64, String> {
    let foreground = luminance(foreground)?;
    let background = luminance(background)?;
    let (lighter, darker) = if foreground >= background {
        (foreground, background)
    } else {
        (background, foreground)
    };
    Ok((lighter + 0.05) / (darker + 0.05))
}

fn luminance(hex: &str) -> Result<f64, String> {
    let value = hex
        .strip_prefix('#')
        .ok_or_else(|| format!("invalid colour {hex}"))?;
    if value.len() != 6 {
        return Err(format!("invalid colour {hex}"));
    }
    let mut channels = [0.0; 3];
    for (index, channel) in channels.iter_mut().enumerate() {
        let start = index * 2;
        let parsed = u8::from_str_radix(&value[start..start + 2], 16)
            .map_err(|_| format!("invalid colour {hex}"))?;
        let srgb = f64::from(parsed) / 255.0;
        *channel = if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        };
    }
    Ok(0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2])
}

/// App settings stored in the caller-provided folder.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Language code.
    pub language: String,
    /// Whether the optional update check is enabled.
    pub check_for_updates: bool,
    /// Storage budget in bytes.
    pub storage_limit: u64,
    /// Trash retention in days.
    pub trash_retention_days: u32,
    /// Optional name shown by the labeller.
    pub labeller_name: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en".into(),
            check_for_updates: false,
            storage_limit: 5 * 1024 * 1024 * 1024,
            trash_retention_days: 30,
            labeller_name: None,
        }
    }
}

/// Load settings, preserving malformed input beside the original and using defaults.
pub fn load(folder: &Path) -> io::Result<Settings> {
    let path = folder.join("settings.json");
    let size = match fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(error) => return Err(error),
    };
    if size > MAX_SETTINGS_BYTES {
        preserve_bad_file(&path)?;
        return Ok(Settings::default());
    }
    let bytes = fs::read(&path)?;
    match serde_json::from_slice(&bytes) {
        Ok(settings) => Ok(settings),
        Err(_) => {
            preserve_bad_file(&path)?;
            Ok(Settings::default())
        }
    }
}

fn preserve_bad_file(path: &Path) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let base = path.file_name().unwrap_or_default().to_string_lossy();
    for suffix in 1..=100 {
        let candidate = parent.join(format!("{base}.bad-{suffix}"));
        // A rename works on USB stick file systems, which have no hard links.
        if !candidate.try_exists()? {
            return fs::rename(path, &candidate);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not preserve bad settings file",
    ))
}

/// Save settings atomically in the caller-provided folder.
pub fn save(folder: &Path, settings: &Settings) -> io::Result<()> {
    fs::create_dir_all(folder)?;
    let bytes = serde_json::to_vec_pretty(settings).map_err(io::Error::other)?;
    kh_store::atomic::WriteOptions::default()
        .with_new_file_permissions(kh_store::atomic::NewFilePermissions::Private)
        .atomic_write(&folder.join("settings.json"), &bytes)
}

/// Keyboard action available through the shared shortcut table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    /// Open the command palette.
    CommandPalette,
    /// Undo the last action.
    Undo,
    /// Redo the last undone action.
    Redo,
    /// Switch to the numbered tool.
    SwitchTool(u8),
}

/// Return the app's keyboard shortcut table.
pub fn shortcuts() -> Vec<(&'static str, ShortcutAction)> {
    let mut table = vec![
        ("Ctrl+K", ShortcutAction::CommandPalette),
        ("Ctrl+Z", ShortcutAction::Undo),
        ("Ctrl+Y", ShortcutAction::Redo),
    ];
    table.extend((1..=6).map(|number| {
        let key = match number {
            1 => "Ctrl+1",
            2 => "Ctrl+2",
            3 => "Ctrl+3",
            4 => "Ctrl+4",
            5 => "Ctrl+5",
            _ => "Ctrl+6",
        };
        (key, ShortcutAction::SwitchTool(number))
    }));
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalogue_has_well_formed_placeholders_and_plurals() {
        let catalogue = english_catalogue().expect("valid bundled JSON");
        validate_catalogue(&catalogue).expect("well formed catalogue");
        assert!(catalogue.contains_key("rail.home"));
    }

    #[test]
    fn translation_uses_english_plural_and_returns_missing_key() {
        assert_eq!(translate("rail.home", None, &BTreeMap::new()), "Home");
        assert_eq!(
            translate("files.count", Some(1), &BTreeMap::new()),
            "1 file"
        );
        assert_eq!(
            translate("files.count", Some(4), &BTreeMap::new()),
            "4 files"
        );
        assert_eq!(
            translate("missing.key", None, &BTreeMap::new()),
            "missing.key"
        );
    }

    #[test]
    fn all_existing_text_pairs_meet_wcag_contrast() {
        let pairs = [
            ("grey on white", BRAND.grey, "#ffffff"),
            ("primary on white", BRAND.primary, "#ffffff"),
            ("tertiary on white", BRAND.tertiary, "#ffffff"),
            ("grey on secondary", BRAND.grey, BRAND.secondary),
            ("grey on yellow", BRAND.grey, BRAND.yellow),
        ];
        let failing: Vec<_> = pairs
            .iter()
            .filter_map(|(name, foreground, background)| {
                let ratio = contrast_ratio(foreground, background).expect("valid brand colours");
                (ratio < 4.5).then_some((*name, ratio))
            })
            .collect();
        assert_eq!(
            failing.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            ["primary on white", "tertiary on white"]
        );
        assert!((failing[0].1 - 2.677_07).abs() < 0.000_01);
        assert!((failing[1].1 - 4.390_16).abs() < 0.000_01);
    }

    #[test]
    fn values_with_braces_are_not_filled_twice() {
        let values = BTreeMap::from([("name".to_owned(), "{count}.csv".to_owned())]);
        assert_eq!(
            fill("{name} has {count} {rows", Some(2), &values),
            "{count}.csv has 2 {rows"
        );
    }

    #[test]
    fn an_oversized_settings_file_is_set_aside() -> io::Result<()> {
        let folder = std::env::temp_dir().join(format!("kh-settings-big-{}", std::process::id()));
        fs::create_dir_all(&folder)?;
        fs::write(folder.join("settings.json"), vec![b' '; 70_000])?;
        assert_eq!(load(&folder)?, Settings::default());
        assert!(folder.join("settings.json.bad-1").exists());
        fs::remove_dir_all(folder)
    }

    #[test]
    fn malformed_catalogue_forms_fail_validation() {
        let mut catalogue = BTreeMap::new();
        catalogue.insert("bad".into(), Message::Text("Hello {name".into()));
        assert!(validate_catalogue(&catalogue).is_err());
        catalogue.insert(
            "plural".into(),
            Message::Plural(BTreeMap::from([("one".into(), "item".into())])),
        );
        assert!(validate_catalogue(&catalogue).is_err());
    }

    #[test]
    fn settings_save_and_load_and_keep_bad_input() -> io::Result<()> {
        let folder = std::env::temp_dir().join(format!("kh-settings-{}", std::process::id()));
        fs::create_dir_all(&folder)?;
        let settings = Settings {
            labeller_name: Some("Example".into()),
            ..Settings::default()
        };
        save(&folder, &settings)?;
        assert_eq!(load(&folder)?, settings);
        fs::write(folder.join("settings.json"), b"not json")?;
        assert_eq!(load(&folder)?, Settings::default());
        assert!(folder.join("settings.json.bad-1").exists());
        fs::remove_dir_all(folder)
    }

    #[test]
    fn shortcut_table_contains_required_keys() {
        let keys: Vec<_> = shortcuts().into_iter().map(|(key, _)| key).collect();
        assert_eq!(
            keys,
            [
                "Ctrl+K", "Ctrl+Z", "Ctrl+Y", "Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4", "Ctrl+5",
                "Ctrl+6"
            ]
        );
    }
}
