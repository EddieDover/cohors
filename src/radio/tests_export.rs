use super::*;
use crate::test_utils::with_xdg_config_home;
use std::fs;
use tempfile::{TempDir, tempdir};

#[test]
fn test_add_station_to_config_new_file() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let station = RadioStation {
        name: "Test Station".to_string(),
        url: "http://test.com".to_string(),
        description: Some("Desc".to_string()),
        homepage: Some("Home".to_string()),
        tags: Some("Tag".to_string()),
        last_playing: None,
    };

    with_xdg_config_home(&config_dir, || {
        add_station_to_config(&station).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();

        assert_eq!(config.radio.individual_stations.len(), 1);
        assert_eq!(config.radio.individual_stations[0].name, "Test Station");
    });
}

#[test]
fn test_add_station_to_config_existing_file() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let initial_config = AppConfig {
        volume: None,
        subsonic: None,
        favorites: Default::default(),
        radio: RadioConfig {
            sources: Vec::new(),
            individual_stations: vec![IndividualStationConfig {
                name: "Existing".to_string(),
                station_url: "http://existing.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
            }],
        },
    };

    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let content = serde_json::to_string(&initial_config).unwrap();
    fs::write(&config_path, content).unwrap();

    let station = RadioStation {
        name: "New Station".to_string(),
        url: "http://new.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    with_xdg_config_home(&config_dir, || {
        add_station_to_config(&station).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();

        assert_eq!(config.radio.individual_stations.len(), 2);
    });
}

#[test]
fn test_add_station_to_config_duplicate() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let station = RadioStation {
        name: "Test Station".to_string(),
        url: "http://test.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    with_xdg_config_home(&config_dir, || {
        add_station_to_config(&station).unwrap();
        add_station_to_config(&station).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();

        assert_eq!(config.radio.individual_stations.len(), 1);
    });
}

#[test]
fn test_delete_station_from_config() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let initial_config = AppConfig {
        volume: None,
        subsonic: None,
        favorites: Default::default(),
        radio: RadioConfig {
            sources: Vec::new(),
            individual_stations: vec![IndividualStationConfig {
                name: "To Delete".to_string(),
                station_url: "http://delete.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
            }],
        },
    };

    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let content = serde_json::to_string(&initial_config).unwrap();
    fs::write(&config_path, content).unwrap();

    with_xdg_config_home(&config_dir, || {
        delete_station_from_config("http://delete.com").unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();

        assert_eq!(config.radio.individual_stations.len(), 0);
    });
}

#[test]
fn test_delete_source_from_config() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let initial_config = AppConfig {
        volume: None,
        subsonic: None,
        favorites: Default::default(),
        radio: RadioConfig {
            sources: vec![RadioSourceConfig {
                title: "To Delete".to_string(),
                json_url: "http://delete.com".to_string(),
                container: None,
                mapping: StationMapping {
                    station_name: "name".to_string(),
                    station_url: "url".to_string(),
                    description: None,
                    homepage: None,
                    tags: None,
                    last_playing: None,
                },
            }],
            individual_stations: Vec::new(),
        },
    };

    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let content = serde_json::to_string(&initial_config).unwrap();
    fs::write(&config_path, content).unwrap();

    with_xdg_config_home(&config_dir, || {
        delete_source_from_config("To Delete").unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();

        assert_eq!(config.radio.sources.len(), 0);
    });
}

#[test]
fn test_edit_station_in_config() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let initial_config = AppConfig {
        volume: None,
        subsonic: None,
        favorites: Default::default(),
        radio: RadioConfig {
            sources: Vec::new(),
            individual_stations: vec![IndividualStationConfig {
                name: "Old Name".to_string(),
                station_url: "http://old.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
            }],
        },
    };

    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let json = serde_json::to_string(&initial_config).unwrap();
    fs::write(&config_path, json).unwrap();

    let new_station = RadioStation {
        name: "New Name".to_string(),
        url: "http://new.com".to_string(),
        description: Some("Desc".to_string()),
        homepage: None,
        tags: None,
        last_playing: None,
    };

    with_xdg_config_home(&config_dir, || {
        edit_station_in_config("http://old.com", &new_station).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();

        assert_eq!(config.radio.individual_stations.len(), 1);
        assert_eq!(config.radio.individual_stations[0].name, "New Name");
        assert_eq!(
            config.radio.individual_stations[0].station_url,
            "http://new.com"
        );
        assert_eq!(
            config.radio.individual_stations[0].description,
            Some("Desc".to_string())
        );
    });
}

#[test]
fn test_edit_source_in_config() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let initial_source = RadioSourceConfig {
        title: "Old Title".to_string(),
        json_url: "http://old.com".to_string(),
        container: None,
        mapping: StationMapping {
            station_name: "n".to_string(),
            station_url: "u".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        },
    };
    let initial_config = AppConfig {
        volume: None,
        subsonic: None,
        favorites: Default::default(),
        radio: RadioConfig {
            sources: vec![initial_source],
            individual_stations: Vec::new(),
        },
    };

    fs::create_dir_all(config_path.parent().unwrap()).unwrap();
    let json = serde_json::to_string(&initial_config).unwrap();
    fs::write(&config_path, json).unwrap();

    let new_source = RadioSourceConfig {
        title: "New Title".to_string(),
        json_url: "http://new.com".to_string(),
        container: Some("data".to_string()),
        mapping: StationMapping {
            station_name: "name".to_string(),
            station_url: "url".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        },
    };

    with_xdg_config_home(&config_dir, || {
        edit_source_in_config("Old Title", &new_source).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();

        assert_eq!(config.radio.sources.len(), 1);
        assert_eq!(config.radio.sources[0].title, "New Title");
        assert_eq!(config.radio.sources[0].json_url, "http://new.com");
        assert_eq!(config.radio.sources[0].container, Some("data".to_string()));
    });
}

#[test]
fn test_add_source_to_config() {
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    let source = RadioSourceConfig {
        title: "New Source".to_string(),
        json_url: "http://example.com/json".to_string(),
        container: None,
        mapping: StationMapping {
            station_name: "name".to_string(),
            station_url: "url".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        },
    };

    with_xdg_config_home(&config_dir, || {
        // Add to new file
        add_source_to_config(&source).unwrap();
        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();
        assert_eq!(config.radio.sources.len(), 1);
        assert_eq!(config.radio.sources[0].title, "New Source");

        // Add duplicate (should be ignored)
        add_source_to_config(&source).unwrap();
        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();
        assert_eq!(config.radio.sources.len(), 1);

        // Add another source
        let mut source2 = source.clone();
        source2.title = "Another Source".to_string();
        source2.json_url = "http://example.com/json2".to_string();
        add_source_to_config(&source2).unwrap();
        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();
        assert_eq!(config.radio.sources.len(), 2);
    });
}
