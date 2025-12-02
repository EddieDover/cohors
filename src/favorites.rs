use crate::radio::RadioStation;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Favorites {
    pub files: Vec<PathBuf>,
    pub stations: Vec<RadioStation>,
}

impl Favorites {
    pub fn toggle_file(&mut self, path: PathBuf) {
        if let Some(index) = self.files.iter().position(|p| *p == path) {
            self.files.remove(index);
        } else {
            self.files.push(path);
        }
    }

    pub fn toggle_station(&mut self, station: RadioStation) {
        if let Some(index) = self.stations.iter().position(|s| s.url == station.url) {
            self.stations.remove(index);
        } else {
            self.stations.push(station);
        }
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
