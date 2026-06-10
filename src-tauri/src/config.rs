use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub window_x: Option<f64>,
    pub window_y: Option<f64>,
    pub colors: ColorConfig,
    pub monitor_directory: Option<String>,
    pub idle_timeout_secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColorConfig {
    pub idle: String,
    pub working: String,
    pub stopped: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            window_x: None,
            window_y: None,
            colors: ColorConfig {
                idle: "#9E9E9E".to_string(),
                working: "#4CAF50".to_string(),
                stopped: "#FFC107".to_string(),
            },
            monitor_directory: None,
            idle_timeout_secs: 60,
        }
    }
}

impl AppConfig {
    pub fn path() -> PathBuf {
        let base = if cfg!(target_os = "windows") {
            std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
        } else {
            std::env::var("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
                .unwrap_or_else(|_| PathBuf::from("."))
        };
        let path = base.join("worm-ai-light");
        std::fs::create_dir_all(&path).ok();
        path.join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|content| toml::from_str(&content).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Ok(content) = toml::to_string_pretty(self) {
            std::fs::write(&path, content).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use std::path::PathBuf;

    #[test]
    fn test_default_idle_timeout() {
        let config = AppConfig::default();
        assert_eq!(config.idle_timeout_secs, 60);
    }

    #[test]
    fn test_default_monitor_directory() {
        let config = AppConfig::default();
        assert!(config.monitor_directory.is_none());
    }

    #[test]
    fn test_config_path_is_toml() {
        let path = AppConfig::path();
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    #[test]
    fn test_toml_roundtrip() {
        let mut config = AppConfig::default();
        config.idle_timeout_secs = 120;
        config.monitor_directory = Some("C:\\test\\dir".to_string());

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.idle_timeout_secs, 120);
        assert_eq!(parsed.monitor_directory, Some("C:\\test\\dir".to_string()));
    }

    #[test]
    fn test_toml_missing_fields_get_defaults() {
        // Simulate an old config file missing idle_timeout_secs
        let toml_str = r##"
window_x = 100.0
window_y = 200.0

[colors]
idle = "#9E9E9E"
working = "#4CAF50"
stopped = "#FFC107"
"##;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.idle_timeout_secs, 60); // from Default
        assert_eq!(config.window_x, Some(100.0));
        assert_eq!(config.window_y, Some(200.0));
    }
}

