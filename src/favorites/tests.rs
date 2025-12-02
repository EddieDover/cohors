use super::*;
use crate::config::AppConfig;
use crate::test_utils::with_xdg_config_home;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_favorites_persistence() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().to_path_buf();

    with_xdg_config_home(&config_dir, || {
        let mut config = AppConfig::default();
        config.favorites.files.push(PathBuf::from("test_file.mp3"));
        let station = RadioStation {
            name: "Test".to_string(),
            url: "http://test.com".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        };
        config.favorites.stations.push(station.clone());

        // Test Save
        assert!(config.save().is_ok());

        // Verify file exists
        let expected_path = config_dir.join("cohors").join("config.json");
        assert!(expected_path.exists());

        // Test Load
        let loaded_config = AppConfig::load().unwrap();
        assert_eq!(loaded_config.favorites.files.len(), 1);
        assert_eq!(loaded_config.favorites.stations.len(), 1);
        assert_eq!(
            loaded_config.favorites.files[0],
            PathBuf::from("test_file.mp3")
        );
        assert_eq!(loaded_config.favorites.stations[0].name, "Test");
    });
}

#[test]
fn test_favorites_load_empty() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().to_path_buf();

    with_xdg_config_home(&config_dir, || {
        let loaded_config = AppConfig::load().unwrap();
        assert!(loaded_config.favorites.files.is_empty());
        assert!(loaded_config.favorites.stations.is_empty());
    });
}

#[test]
fn test_favorites_load_corrupted() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().to_path_buf();

    with_xdg_config_home(&config_dir, || {
        let cohors_dir = config_dir.join("cohors");
        fs::create_dir_all(&cohors_dir).unwrap();
        let config_path = cohors_dir.join("config.json");
        fs::write(&config_path, "{ invalid json").unwrap();

        let result = AppConfig::load();
        assert!(result.is_err());
    });
}
