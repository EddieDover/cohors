use super::*;
use crate::test_utils::with_xdg_config_home;
use tempfile::tempdir;

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

#[test]
fn test_favorites_load_corrupted() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().to_path_buf();

    with_xdg_config_home(&config_path, || {
        let cohors_dir = config_path.join("cohors");
        fs::create_dir_all(&cohors_dir).unwrap();
        let favorites_path = cohors_dir.join("favorites.json");
        fs::write(&favorites_path, "{ invalid json").unwrap();

        let loaded = Favorites::load();
        assert!(loaded.files.is_empty());
        assert!(loaded.stations.is_empty());
    });
}
