use super::*;
use std::fs;
use tempfile::TempDir;

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
