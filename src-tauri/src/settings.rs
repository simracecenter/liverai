//! App settings: one JSON file in the app config directory.
//!
//! Settings never hold a credential. Claude sign-in stays inside the Driver's
//! own Claude Code install.

use crate::error::{CommandError, CommandResult};
use crate::fsutil::write_atomic;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "settings.json";
pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
pub const DEFAULT_EFFORT: &str = "high";
pub const EFFORT_LEVELS: [&str; 3] = ["low", "medium", "high"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// iRacing paint folder. Empty means "not resolved"; the default is
    /// `Documents\iRacing\paint`.
    pub paint_folder: String,
    /// iRacing customer id, digits only. Empty until the Driver sets it.
    pub customer_id: String,
    /// Claude model id passed to the Claude Code CLI.
    pub model: String,
    /// Effort level: low, medium or high.
    pub effort: String,
    pub ask_before_install: bool,
    pub ask_before_input: bool,
    pub backup_before_install: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paint_folder: default_paint_folder()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            customer_id: String::new(),
            model: DEFAULT_MODEL.to_string(),
            effort: DEFAULT_EFFORT.to_string(),
            ask_before_install: true,
            ask_before_input: true,
            backup_before_install: true,
        }
    }
}

/// `Documents\iRacing\paint` under the Driver's Documents known folder.
pub fn default_paint_folder() -> Option<PathBuf> {
    dirs::document_dir().map(|d| d.join("iRacing").join("paint"))
}

/// A customer id is empty (not set) or up to 12 ASCII digits.
pub fn validate_customer_id(id: &str) -> CommandResult<()> {
    if id.len() > 12 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return Err(CommandError::invalid(
            "Customer ID must be digits only, 12 at most.",
        ));
    }
    Ok(())
}

/// A model id is passed to the CLI as an argument, so it must look like an
/// id and can never be read as a flag.
pub fn validate_model(model: &str) -> CommandResult<()> {
    let ok = !model.is_empty()
        && model.len() <= 64
        && model.as_bytes()[0].is_ascii_alphanumeric()
        && model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'[' | b']'));
    if !ok {
        return Err(CommandError::invalid("Model id is not valid."));
    }
    Ok(())
}

impl Settings {
    /// Trim text fields and reject values the core cannot use.
    pub fn normalised(mut self) -> CommandResult<Self> {
        self.paint_folder = self.paint_folder.trim().to_string();
        self.customer_id = self.customer_id.trim().to_string();
        self.model = self.model.trim().to_string();
        self.effort = self.effort.trim().to_ascii_lowercase();

        if self.paint_folder.is_empty() {
            return Err(CommandError::invalid("Paint folder is required."));
        }
        if self.paint_folder.len() > 1024 || self.paint_folder.contains('\0') {
            return Err(CommandError::invalid("Paint folder path is not valid."));
        }
        validate_customer_id(&self.customer_id)?;
        validate_model(&self.model)?;
        if !EFFORT_LEVELS.contains(&self.effort.as_str()) {
            return Err(CommandError::invalid("Effort must be low, medium or high."));
        }
        Ok(self)
    }
}

/// Load settings from `dir`. A missing or unreadable file yields the defaults;
/// missing fields take their default value.
pub fn load(dir: &Path) -> Settings {
    let mut settings = fs::read_to_string(dir.join(SETTINGS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .unwrap_or_default();
    if settings.paint_folder.trim().is_empty() {
        settings.paint_folder = Settings::default().paint_folder;
    }
    settings
}

/// Validate and save settings to `dir`, creating it if needed. Returns the
/// normalised settings that were written.
pub fn save(dir: &Path, settings: Settings) -> CommandResult<Settings> {
    let settings = settings.normalised()?;
    fs::create_dir_all(dir).map_err(|e| CommandError::io("Create settings folder", &e))?;
    let json = serde_json::to_vec_pretty(&settings)
        .map_err(|e| CommandError::data("Serialise settings", &e))?;
    write_atomic(&dir.join(SETTINGS_FILE), &json)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Settings {
        Settings {
            paint_folder: r"C:\Users\driver\Documents\iRacing\paint".to_string(),
            ..Settings::default()
        }
    }

    #[test]
    fn defaults_are_safe() {
        let s = Settings::default();
        assert_eq!(s.model, "claude-opus-5-5");
        assert_eq!(s.effort, "high");
        assert_eq!(s.customer_id, "");
        assert!(s.ask_before_install);
        assert!(s.ask_before_input);
        assert!(s.backup_before_install);
    }

    #[test]
    fn load_returns_defaults_when_missing_or_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Settings::default());
        fs::write(dir.path().join(SETTINGS_FILE), "{not json").unwrap();
        assert_eq!(load(dir.path()), Settings::default());
    }

    #[test]
    fn load_fills_missing_fields_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"customer_id":"123456","ask_before_install":false}"#,
        )
        .unwrap();
        let s = load(dir.path());
        assert_eq!(s.customer_id, "123456");
        assert!(!s.ask_before_install);
        assert!(s.ask_before_input);
        assert_eq!(s.model, DEFAULT_MODEL);
    }

    #[test]
    fn save_round_trips_and_normalises() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("config");
        let saved = save(
            &nested,
            Settings {
                customer_id: " 123456 ".to_string(),
                effort: "Medium".to_string(),
                model: "claude-fable-5-2".to_string(),
                ..valid()
            },
        )
        .unwrap();
        assert_eq!(saved.customer_id, "123456");
        assert_eq!(saved.effort, "medium");
        assert_eq!(load(&nested), saved);
    }

    #[test]
    fn rejects_bad_values() {
        let cases = [
            Settings {
                customer_id: "12a4".into(),
                ..valid()
            },
            Settings {
                customer_id: "1234567890123".into(),
                ..valid()
            },
            Settings {
                effort: "ludicrous".into(),
                ..valid()
            },
            Settings {
                model: "--dangerously-skip-permissions".into(),
                ..valid()
            },
            Settings {
                model: "claude opus".into(),
                ..valid()
            },
            Settings {
                model: String::new(),
                ..valid()
            },
            Settings {
                paint_folder: "   ".into(),
                ..valid()
            },
        ];
        for case in cases {
            let err = case.clone().normalised().unwrap_err();
            assert_eq!(err.code, "invalid_input", "{case:?}");
        }
    }
}
