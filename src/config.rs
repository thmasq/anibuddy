use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct PresetConfig {
    pub path: String,
    pub fps: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub default: Option<PresetConfig>,
    #[serde(flatten)]
    pub presets: HashMap<String, PresetConfig>,
}

impl Config {
    pub fn load() -> Result<Option<Self>> {
        let config_path = get_config_path()?;

        if !config_path.exists() {
            log::debug!("Config file not found at {}", config_path.display());
            return Ok(None);
        }

        log::info!("Loading config from {}", config_path.display());

        let config_content = fs::read_to_string(&config_path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;

        let config: Config = toml::from_str(&config_content)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;

        log::debug!("Loaded config with {} presets", config.presets.len());

        Ok(Some(config))
    }

    pub fn get_preset(&self, name: &str) -> Option<&PresetConfig> {
        // Special case for "default"
        if name == "default" {
            return self.default.as_ref();
        }

        self.presets.get(name)
    }

    pub fn get_default(&self) -> Option<&PresetConfig> {
        self.default.as_ref()
    }

    pub fn list_presets(&self) -> Vec<String> {
        let mut presets = Vec::new();

        if self.default.is_some() {
            presets.push("default".to_string());
        }

        let mut preset_names: Vec<_> = self.presets.keys().cloned().collect();
        preset_names.sort();
        presets.extend(preset_names);

        presets
    }
}

fn get_config_path() -> Result<PathBuf> {
    let home_dir = dirs::home_dir()
        .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
        .ok_or_else(|| anyhow!("Could not determine home directory"))?;

    Ok(home_dir
        .join(".config")
        .join("anibuddy")
        .join("config.toml"))
}

pub fn is_likely_path(input: &str) -> bool {
    // Check if the input looks like a file path rather than a preset name
    input.contains('/')
        || input.contains('\\')
        || input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with("~/")
        || input.contains('.') && input.len() > 1 // Has extension or is relative path like "./dir"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_path() {
        // These should be treated as paths
        assert!(is_likely_path("./frames"));
        assert!(is_likely_path("../frames"));
        assert!(is_likely_path("~/frames"));
        assert!(is_likely_path("/home/user/frames"));
        assert!(is_likely_path("frames/subfolder"));
        assert!(is_likely_path("animation.gif"));
        assert!(is_likely_path("frames.png"));
        assert!(is_likely_path("C:\\frames"));

        // These should be treated as preset names
        assert!(!is_likely_path("frames"));
        assert!(!is_likely_path("konata"));
        assert!(!is_likely_path("1"));
        assert!(!is_likely_path("dancer"));
        assert!(!is_likely_path("default"));
    }
}
