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

        if favorites_path.exists()
            && let Ok(content) = fs::read_to_string(favorites_path)
            && let Ok(favorites) = serde_json::from_str(&content)
        {
            return favorites;
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
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // Helper to run test with modified environment
    fn with_xdg_config_home<F>(path: &std::path::Path, f: F)
    where
        F: FnOnce(),
    {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let key = "XDG_CONFIG_HOME";
        let old_val = env::var_os(key);
        unsafe {
            env::set_var(key, path);
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        unsafe {
            if let Some(val) = old_val {
                env::set_var(key, val);
            } else {
                env::remove_var(key);
            }
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn test_favorites_persistence() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().to_path_buf();

        with_xdg_config_home(&config_path, || {
            let mut favs = Favorites::default();
            favs.files.push(PathBuf::from("test_file.mp3"));
            let station = RadioStation {
                name: "Test".to_string(),
                url: "http://test.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
                last_playing: None,
            };
            favs.stations.push(station.clone());

            // Test Save
            assert!(favs.save().is_ok());

            // Verify file exists
            let expected_path = config_path.join("cohors").join("favorites.json");
            assert!(expected_path.exists());

            // Test Load
            let loaded = Favorites::load();
            assert_eq!(loaded.files.len(), 1);
            assert_eq!(loaded.stations.len(), 1);
            assert_eq!(loaded.files[0], PathBuf::from("test_file.mp3"));
            assert_eq!(loaded.stations[0].name, "Test");
        });
    }

    #[test]
    fn test_favorites_load_empty() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().to_path_buf();

        with_xdg_config_home(&config_path, || {
            let loaded = Favorites::load();
            assert!(loaded.files.is_empty());
            assert!(loaded.stations.is_empty());
        });
    }
}
