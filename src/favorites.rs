use crate::radio::RadioStation;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Favorites {
    pub files: Vec<PathBuf>,
    pub stations: Vec<RadioStation>,
}

impl Favorites {
    pub fn load() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let favorites_path = config_dir.join("cohors").join("favorites.json");

        if favorites_path.exists() {
            match fs::read_to_string(&favorites_path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(favorites) => return favorites,
                    Err(e) => {
                        eprintln!("Failed to parse favorites: {}", e);
                    }
                },
                Err(e) => eprintln!("Failed to read favorites file: {}", e),
            }
        }

        Favorites::default()
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let cohors_dir = config_dir.join("cohors");
        if !cohors_dir.exists() {
            fs::create_dir_all(&cohors_dir)?;
        }
        let favorites_path = cohors_dir.join("favorites.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(favorites_path, content)?;
        Ok(())
    }

    pub fn toggle_file(&mut self, path: PathBuf) {
        if let Some(index) = self.files.iter().position(|p| *p == path) {
            self.files.remove(index);
        } else {
            self.files.push(path);
        }
        let _ = self.save();
    }

    pub fn toggle_station(&mut self, station: RadioStation) {
        if let Some(index) = self.stations.iter().position(|s| s.url == station.url) {
            self.stations.remove(index);
        } else {
            self.stations.push(station);
        }
        let _ = self.save();
    }

    pub fn is_favorite_file(&self, path: &PathBuf) -> bool {
        self.files.contains(path)
    }

    pub fn is_favorite_station(&self, station: &RadioStation) -> bool {
        self.stations.iter().any(|s| s.url == station.url)
    }
}

#[cfg(test)]
mod tests;
