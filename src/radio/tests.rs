use super::*;
use tempfile::tempdir;

#[test]
fn test_parse_pls() {
    let pls_content = "[playlist]\nNumberOfEntries=1\nFile1=http://example.com/stream\nTitle1=Example Radio\nLength1=-1\nVersion=2";
    let url = parse_pls(pls_content).unwrap();
    assert_eq!(url, "http://example.com/stream");
}

#[test]
fn test_get_string_field() {
    let json = serde_json::json!({
        "title": "Test Station",
        "playlists": [
            { "url": "http://test.com/stream" }
        ]
    });

    assert_eq!(get_string_field(&json, "title").unwrap(), "Test Station");
    assert_eq!(
        get_string_field(&json, "playlists.0.url").unwrap(),
        "http://test.com/stream"
    );
}

#[test]
fn test_map_station_fallback() {
    let mapping = StationMapping {
        station_name: "name".to_string(),
        station_url: "url".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    // Case 1: Name is present
    let item = serde_json::json!({
        "name": "My Station",
        "url": "http://test.com"
    });
    let station = map_station(&item, &mapping).unwrap();
    assert_eq!(station.name, "My Station");

    // Case 2: Name is empty -> Fallback to URL
    let item = serde_json::json!({
        "name": "   ",
        "url": "http://test.com"
    });
    let station = map_station(&item, &mapping).unwrap();
    assert_eq!(station.name, "http://test.com");
}

#[test]
fn test_map_station_all_fields() {
    let mapping = StationMapping {
        station_name: "n".to_string(),
        station_url: "u".to_string(),
        description: Some("d".to_string()),
        homepage: Some("h".to_string()),
        tags: Some("t".to_string()),
        last_playing: Some("l".to_string()),
    };

    let item = serde_json::json!({
        "n": "Name",
        "u": "Url",
        "d": "Desc",
        "h": "Home",
        "t": "Tags",
        "l": "Last"
    });

    let station = map_station(&item, &mapping).unwrap();
    assert_eq!(station.name, "Name");
    assert_eq!(station.url, "Url");
    assert_eq!(station.description.as_deref(), Some("Desc"));
    assert_eq!(station.homepage.as_deref(), Some("Home"));
    assert_eq!(station.tags.as_deref(), Some("Tags"));
    assert_eq!(station.last_playing.as_deref(), Some("Last"));
}

#[test]
fn test_load_config_home() {
    let temp_home = tempdir().unwrap();
    let config_dir = temp_home.path().join(".config/cohors");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("stations.config.json");

    let config_content = r#"{ "sources": [] }"#;
    std::fs::write(&config_path, config_content).unwrap();

    let config = load_config(None, Some(temp_home.path().to_path_buf()), None).unwrap();
    assert!(config.sources.is_empty());
}

#[tokio::test]
async fn test_fetch_all_stations() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("stations.config.json");

    // Mock server
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(200)
        .with_body(r#"[{"name": "Station 1", "url": "http://1.com"}]"#)
        .create_async()
        .await;

    let config_content = format!(
        r#"{{
        "sources": [
            {{
                "title": "Test Source",
                "json_url": "{}/stations.json",
                "mapping": {{
                    "station_name": "name",
                    "station_url": "url"
                }}
            }}
        ]
    }}"#,
        server.url()
    );

    std::fs::write(&config_path, config_content).unwrap();

    let groups = fetch_all_stations(Some(config_path), None, true, true)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].title, "Test Source");
    assert_eq!(groups[0].stations.len(), 1);
}

#[tokio::test]
async fn test_fetch_stations_caching() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"name": "Station 1", "url": "http://1.com"}]"#)
        .create_async()
        .await;

    let temp_home = tempdir().unwrap();

    let source = RadioSourceConfig {
        title: "TestSource".to_string(),
        json_url: format!("{}/stations.json", server.url()),
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

    // First fetch: Should hit web (mock) and save to cache
    let stations = fetch_stations(&source, Some(temp_home.path().to_path_buf()), false, true)
        .await
        .unwrap();
    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0].name, "Station 1");

    // Check if cache file exists
    let cache_path = temp_home
        .path()
        .join(".cache/cohors/stations/TestSource.json");
    assert!(cache_path.exists());

    // Modify cache file to verify second fetch reads from it
    let cached_content = r#"[{"name": "Cached Station", "url": "http://cached.com"}]"#;
    fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
    fs::write(&cache_path, cached_content).unwrap();

    // Second fetch: Should hit cache
    let stations = fetch_stations(&source, Some(temp_home.path().to_path_buf()), false, true)
        .await
        .unwrap();
    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0].name, "Cached Station");
}

#[test]
fn test_parse_m3u() {
    let m3u_content = "#EXTM3U\n#EXTINF:-1,Example Radio\nhttp://example.com/stream\n#EXTINF:-1,Another Stream\nhttp://example.com/stream2";
    let url = parse_m3u(m3u_content).unwrap();
    assert_eq!(url, "http://example.com/stream");
}

#[test]
fn test_fetch_playlist_stream_url_check() {
    let mut server = mockito::Server::new();

    // Mock audio stream response
    let _m_audio = server
        .mock("GET", "/stream")
        .with_status(200)
        .with_header("content-type", "audio/mpeg")
        .with_body("binary audio data")
        .create();

    // Mock valid PLS response
    let _m_pls = server
        .mock("GET", "/playlist.pls")
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("[playlist]\nFile1=http://stream.com")
        .create();

    // Mock valid M3U response
    let _m_m3u = server
        .mock("GET", "/playlist.m3u")
        .with_status(200)
        .with_header("content-type", "audio/x-mpegurl")
        .with_body("#EXTM3U\nhttp://stream.com/m3u")
        .create();

    let client = reqwest::blocking::Client::new();

    // Test audio stream rejection
    let url = format!("{}/stream", server.url());
    let result = fetch_playlist_stream_url(&client, &url);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "URL is an audio stream, not a playlist file"
    );

    // Test valid PLS
    let url = format!("{}/playlist.pls", server.url());
    let result = fetch_playlist_stream_url(&client, &url);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "http://stream.com");

    // Test valid M3U
    let url = format!("{}/playlist.m3u", server.url());
    let result = fetch_playlist_stream_url(&client, &url);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "http://stream.com/m3u");
}

#[test]
fn test_load_config_local() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("stations.config.json");
    let content = r#"{
        "sources": [
            {
                "title": "Test Group",
                "json_url": "http://test.com",
                "container": null,
                "mapping": {
                    "station_name": "name",
                    "station_url": "url"
                }
            }
        ]
    }"#;
    fs::write(&file_path, content).unwrap();

    // Pass current_dir explicitly, and mock home_dir to avoid picking up real user config
    let configs = load_config(
        None,
        Some(dir.path().to_path_buf()),
        Some(dir.path().to_path_buf()),
    )
    .unwrap();
    assert_eq!(configs.sources.len(), 1);
    assert_eq!(configs.sources[0].title, "Test Group");
}

#[test]
fn test_load_config_custom_path() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("custom_stations.json");
    let content = r#"{
        "sources": [
            {
                "title": "Custom Group",
                "json_url": "http://custom.com",
                "container": null,
                "mapping": {
                    "station_name": "name",
                    "station_url": "url"
                }
            }
        ]
    }"#;
    fs::write(&file_path, content).unwrap();

    let configs = load_config(Some(file_path), None, None).unwrap();
    assert_eq!(configs.sources.len(), 1);
    assert_eq!(configs.sources[0].title, "Custom Group");
}

#[test]
fn test_load_config_missing_file() {
    let dir = tempdir().unwrap();

    // Pass temp dir as HOME and CWD, ensuring no config exists there
    let result = load_config(
        None,
        Some(dir.path().to_path_buf()),
        Some(dir.path().to_path_buf()),
    );
    if result.is_ok() {
        println!("Unexpected Ok result: {:?}", result.as_ref().unwrap());
    }
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}
#[test]
fn test_load_config_custom_path_missing() {
    let result = load_config(Some(PathBuf::from("/non/existent/path.json")), None, None);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Custom config file not found")
    );
}

#[tokio::test]
async fn test_fetch_stations_error() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(500)
        .create_async()
        .await;

    let source = RadioSourceConfig {
        title: "ErrorSource".to_string(),
        json_url: format!("{}/stations.json", server.url()),
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

    let result = fetch_stations(&source, None, false, true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_stations_invalid_json() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(200)
        .with_body("invalid json")
        .create_async()
        .await;

    let source = RadioSourceConfig {
        title: "InvalidSource".to_string(),
        json_url: format!("{}/stations.json", server.url()),
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

    let result = fetch_stations(&source, None, false, true).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_fetch_all_stations_error_handling() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join("stations.config.json");

    // Mock server that returns 500
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(500)
        .create_async()
        .await;

    let config_content = format!(
        r#"{{
        "sources": [
            {{
                "title": "Error Source",
                "json_url": "{}/stations.json",
                "mapping": {{
                    "station_name": "name",
                    "station_url": "url"
                }}
            }}
        ]
    }}"#,
        server.url()
    );

    std::fs::write(&config_path, config_content).unwrap();

    // Should not fail, but return empty list (or list with other successful sources)
    let groups = fetch_all_stations(Some(config_path), None, true, true)
        .await
        .unwrap();
    assert_eq!(groups.len(), 0);
}

#[tokio::test]
async fn test_fetch_stations_cache_invalidation() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(200)
        .with_body(r#"[{"name": "Station 1", "url": "http://1.com"}]"#)
        .create_async()
        .await;

    let temp_home = tempdir().unwrap();
    let cache_dir = temp_home.path().join(".cache/cohors/stations");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let cache_file = cache_dir.join("Test_Source.json");
    std::fs::write(&cache_file, "old data").unwrap();

    let source = RadioSourceConfig {
        title: "Test Source".to_string(),
        json_url: format!("{}/stations.json", server.url()),
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

    // Fetch with invalidate_cache = true
    let stations = fetch_stations(&source, Some(temp_home.path().to_path_buf()), true, true)
        .await
        .unwrap();

    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0].name, "Station 1");

    // Cache file should have been overwritten with new data
    let content = std::fs::read_to_string(cache_file).unwrap();
    assert!(content.contains("Station 1"));
}

#[tokio::test]
async fn test_fetch_stations_cache_expiry() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(200)
        .with_body(r#"[{"name": "New Station", "url": "http://new.com"}]"#)
        .create_async()
        .await;

    let temp_home = tempdir().unwrap();
    let cache_dir = temp_home.path().join(".cache/cohors/stations");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let cache_file = cache_dir.join("Test_Source.json");

    // Write old cache file (8 days old)
    std::fs::write(
        &cache_file,
        r#"[{"name": "Old Station", "url": "http://old.com"}]"#,
    )
    .unwrap();

    let file = std::fs::File::open(&cache_file).unwrap();
    let old_time = SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
    file.set_modified(old_time).unwrap();

    let source = RadioSourceConfig {
        title: "Test Source".to_string(),
        json_url: format!("{}/stations.json", server.url()),
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

    // Fetch should ignore cache because it's old
    let stations = fetch_stations(&source, Some(temp_home.path().to_path_buf()), false, true)
        .await
        .unwrap();

    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0].name, "New Station");
}

#[test]
fn test_load_config_individual_stations() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("individual.json");
    let content = r#"{
        "stations": [
            {
                "name": "My Station",
                "station_url": "http://mystation.com",
                "description": "My Description"
            }
        ]
    }"#;
    fs::write(&file_path, content).unwrap();

    let configs = load_config(Some(file_path), None, None).unwrap();
    assert_eq!(configs.sources.len(), 0);
    assert_eq!(configs.individual_stations.len(), 1);
    assert_eq!(configs.individual_stations[0].name, "My Station");
    assert_eq!(
        configs.individual_stations[0].station_url,
        "http://mystation.com"
    );
    assert_eq!(
        configs.individual_stations[0].description,
        Some("My Description".to_string())
    );
}

#[tokio::test]
async fn test_fetch_all_stations_individual() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("individual.json");
    let content = r#"{
        "stations": [
            {
                "name": "My Station",
                "station_url": "http://mystation.com"
            }
        ]
    }"#;
    fs::write(&file_path, content).unwrap();

    let groups = fetch_all_stations(Some(file_path), None, false, true)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].title, "Custom Stations");
    assert_eq!(groups[0].stations.len(), 1);
    assert_eq!(groups[0].stations[0].name, "My Station");
}

#[test]
fn test_get_string_field_types() {
    let json = serde_json::json!({
        "string": "value",
        "number": 123,
        "float": 45.67,
        "bool": true,
        "null": null,
        "object": {},
        "array": []
    });

    assert_eq!(get_string_field(&json, "string").unwrap(), "value");
    assert_eq!(get_string_field(&json, "number").unwrap(), "123");
    assert!(get_string_field(&json, "bool").is_none());
    assert!(get_string_field(&json, "null").is_none());
    assert!(get_string_field(&json, "object").is_none());
    assert!(get_string_field(&json, "array").is_none());
}

#[tokio::test]
async fn test_fetch_stations_cache_future_timestamp() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(200)
        .with_body(r#"[{"name": "New Station", "url": "http://new.com"}]"#)
        .create_async()
        .await;

    let temp_home = tempdir().unwrap();
    let cache_dir = temp_home.path().join(".cache/cohors/stations");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let cache_file = cache_dir.join("Test_Source.json");

    // Write cache file
    std::fs::write(
        &cache_file,
        r#"[{"name": "Cached Station", "url": "http://cached.com"}]"#,
    )
    .unwrap();

    // Set modification time to future
    let file = std::fs::File::open(&cache_file).unwrap();
    let future_time = SystemTime::now() + Duration::from_secs(3600);
    file.set_modified(future_time).unwrap();

    let source = RadioSourceConfig {
        title: "Test Source".to_string(),
        json_url: format!("{}/stations.json", server.url()),
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

    // Fetch should use cache because it's "fresh" (future timestamp)
    let stations = fetch_stations(&source, Some(temp_home.path().to_path_buf()), false, true)
        .await
        .unwrap();

    assert_eq!(stations.len(), 1);
    assert_eq!(stations[0].name, "Cached Station");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_fetch_stations_no_home_dir() {
    let mut server = mockito::Server::new_async().await;
    let _m = server
        .mock("GET", "/stations.json")
        .with_status(200)
        .with_body(r#"[{"name": "Station 1", "url": "http://1.com"}]"#)
        .create_async()
        .await;

    let source = RadioSourceConfig {
        title: "NoHome".to_string(),
        json_url: format!("{}/stations.json", server.url()),
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

    let _guard = crate::test_utils::ENV_MUTEX.lock().unwrap();
    unsafe {
        std::env::remove_var("HOME");
    }

    let stations = fetch_stations(&source, None, false, true).await.unwrap();
    assert_eq!(stations.len(), 1);

    // Restore HOME if needed? The mutex protects other tests.
}

#[tokio::test]
async fn test_fetch_stations_invalid_container() {
    let mut server = mockito::Server::new_async().await;

    // Case 1: Container specified but missing in JSON
    let _m1 = server
        .mock("GET", "/missing.json")
        .with_status(200)
        .with_body(r#"{"other": []}"#)
        .create_async()
        .await;

    let source_missing = RadioSourceConfig {
        title: "Missing".to_string(),
        json_url: format!("{}/missing.json", server.url()),
        container: Some("stations".to_string()),
        mapping: StationMapping {
            station_name: "n".to_string(),
            station_url: "u".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        },
    };
    assert!(
        fetch_stations(&source_missing, None, false, true)
            .await
            .is_err()
    );

    // Case 2: Container specified but not an array
    let _m2 = server
        .mock("GET", "/not_array.json")
        .with_status(200)
        .with_body(r#"{"stations": "invalid"}"#)
        .create_async()
        .await;

    let source_not_array = RadioSourceConfig {
        title: "NotArray".to_string(),
        json_url: format!("{}/not_array.json", server.url()),
        container: Some("stations".to_string()),
        mapping: StationMapping {
            station_name: "n".to_string(),
            station_url: "u".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        },
    };
    assert!(
        fetch_stations(&source_not_array, None, false, true)
            .await
            .is_err()
    );

    // Case 3: No container, root is not array
    let _m3 = server
        .mock("GET", "/root_obj.json")
        .with_status(200)
        .with_body(r#"{"stations": []}"#)
        .create_async()
        .await;

    let source_root = RadioSourceConfig {
        title: "RootObj".to_string(),
        json_url: format!("{}/root_obj.json", server.url()),
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
    assert!(
        fetch_stations(&source_root, None, false, true)
            .await
            .is_err()
    );
}
