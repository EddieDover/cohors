use super::*;
use std::fs;
use tempfile::{tempdir, TempDir};

#[test]
fn test_resolve_config_path_custom() {
    let custom = PathBuf::from("/tmp/custom.json");
    let result = resolve_config_path(Some(custom.clone()), None, None);
    assert_eq!(result, custom);
}

#[test]
fn test_resolve_config_path_home() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().to_path_buf();
    let config_dir = home.join(".config/cohors");
    fs::create_dir_all(&config_dir).unwrap();
    let config_file = config_dir.join("stations.config.json");
    fs::write(&config_file, "{}").unwrap();

    let result = resolve_config_path(None, Some(home.clone()), None);
    assert_eq!(result, config_file);
}

#[test]
fn test_resolve_config_path_local() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();
    let config_file = cwd.join("stations.config.json");
    fs::write(&config_file, "{}").unwrap();

    // Pass a dummy home so it doesn't pick up the real user's config
    let dummy_home = TempDir::new().unwrap().path().to_path_buf();

    let result = resolve_config_path(None, Some(dummy_home), Some(cwd.clone()));
    assert_eq!(result, config_file);
}

#[test]
fn test_resolve_config_path_default_home() {
    let temp = TempDir::new().unwrap();
    let home = temp.path().to_path_buf();

    // Pass a dummy cwd so it doesn't pick up the repo's config
    let dummy_cwd = TempDir::new().unwrap().path().to_path_buf();

    let result = resolve_config_path(None, Some(home.clone()), Some(dummy_cwd));
    assert_eq!(result, home.join(".config/cohors/stations.config.json"));
}

#[test]
fn test_resolve_config_path_fallback() {
    let temp = TempDir::new().unwrap();
    let cwd = temp.path().to_path_buf();

    // Pass a dummy home so it doesn't pick up the real user's config
    let dummy_home = TempDir::new().unwrap().path().to_path_buf();

    let result = resolve_config_path(None, Some(dummy_home.clone()), Some(cwd.clone()));
    // It defaults to home config if nothing exists
    assert_eq!(
        result,
        dummy_home.join(".config/cohors/stations.config.json")
    );
}

#[test]
fn test_add_station_to_config_new_file() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");

    let station = RadioStation {
        name: "Test Station".to_string(),
        url: "http://test.com".to_string(),
        description: Some("Desc".to_string()),
        homepage: Some("Home".to_string()),
        tags: Some("Tag".to_string()),
        last_playing: None,
    };

    add_station_to_config(&config_path, &station).unwrap();

    let content = fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();

    assert_eq!(config.individual_stations.len(), 1);
    assert_eq!(config.individual_stations[0].name, "Test Station");
}

#[test]
fn test_add_station_to_config_existing_file() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");

    let initial_config = RadioConfig {
        sources: Vec::new(),
        individual_stations: vec![IndividualStationConfig {
            name: "Existing".to_string(),
            station_url: "http://existing.com".to_string(),
            description: None,
            homepage: None,
            tags: None,
        }],
    };
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

    add_station_to_config(&config_path, &station).unwrap();

    let content = fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();

    assert_eq!(config.individual_stations.len(), 2);
}

#[test]
fn test_add_station_to_config_duplicate() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");

    let station = RadioStation {
        name: "Test Station".to_string(),
        url: "http://test.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    add_station_to_config(&config_path, &station).unwrap();
    add_station_to_config(&config_path, &station).unwrap();

    let content = fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();

    assert_eq!(config.individual_stations.len(), 1);
}

#[test]
fn test_edit_station_in_config() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");

    let initial_config = RadioConfig {
        sources: Vec::new(),
        individual_stations: vec![IndividualStationConfig {
            name: "Old Name".to_string(),
            station_url: "http://old.com".to_string(),
            description: None,
            homepage: None,
            tags: None,
        }],
    };
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

    edit_station_in_config(&config_path, "http://old.com", &new_station).unwrap();

    let content = fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();

    assert_eq!(config.individual_stations.len(), 1);
    assert_eq!(config.individual_stations[0].name, "New Name");
    assert_eq!(config.individual_stations[0].station_url, "http://new.com");
    assert_eq!(config.individual_stations[0].description, Some("Desc".to_string()));
}

#[test]
fn test_edit_source_in_config() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");

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
    let initial_config = RadioConfig {
        sources: vec![initial_source],
        individual_stations: Vec::new(),
    };
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

    edit_source_in_config(&config_path, "Old Title", &new_source).unwrap();

    let content = fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();

    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.sources[0].title, "New Title");
    assert_eq!(config.sources[0].json_url, "http://new.com");
    assert_eq!(config.sources[0].container, Some("data".to_string()));
}

#[test]
fn test_add_source_to_config() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("stations.config.json");

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

    // Add to new file
    add_source_to_config(&config_path, &source).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.sources[0].title, "New Source");

    // Add duplicate (should be ignored)
    add_source_to_config(&config_path, &source).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(config.sources.len(), 1);

    // Add another source
    let mut source2 = source.clone();
    source2.title = "Another Source".to_string();
    source2.json_url = "http://example.com/json2".to_string();
    add_source_to_config(&config_path, &source2).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    let config: RadioConfig = serde_json::from_str(&content).unwrap();
    assert_eq!(config.sources.len(), 2);
}
