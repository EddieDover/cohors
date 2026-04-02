use crate::favorites::Favorites;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct AppConfig {
    pub volume: Option<f32>,
    #[serde(default)]
    pub radio: RadioConfig,
    #[serde(default)]
    pub favorites: Favorites,
    #[serde(default)]
    pub navidrome: Option<NavidromeConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NavidromeConfig {
    pub sources: Vec<NavidromeSourceConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NavidromeSourceConfig {
    pub server_url: String,
    pub username: String,
    pub password: Option<String>,
    pub auth_token: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RadioConfig {
    #[serde(default)]
    pub sources: Vec<RadioSourceConfig>,
    #[serde(default, rename = "stations")]
    pub individual_stations: Vec<IndividualStationConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IndividualStationConfig {
    pub name: String,
    pub station_url: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RadioSourceConfig {
    pub title: String,
    pub json_url: String,
    pub container: Option<String>,
    pub mapping: StationMapping,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StationMapping {
    pub station_name: String,
    pub station_url: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub tags: Option<String>,
    #[serde(rename = "lastPlaying")]
    pub last_playing: Option<String>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = get_config_path();
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&get_config_path())
    }

    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

fn get_config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("cohors/config.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/cohors/config.json")
    } else {
        PathBuf::from("config.json")
    }
}

pub fn delete_navidrome_from_config(server_url: &str) -> Result<()> {
    let mut config = AppConfig::load()?;
    if let Some(navidrome) = &mut config.navidrome
        && let Some(idx) = navidrome.sources.iter().position(|s| s.server_url == server_url) {
            navidrome.sources.remove(idx);
            config.save()?;
        }
    Ok(())
}
