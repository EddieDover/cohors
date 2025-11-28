use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct RadioGroup {
    pub title: String,
    pub stations: Vec<RadioStation>,
    pub is_expanded: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RadioStation {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub tags: Option<String>,
    pub last_playing: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RadioConfig {
    pub sources: Vec<RadioSourceConfig>,
}

#[derive(Debug, Deserialize)]
pub struct RadioSourceConfig {
    pub title: String,
    pub json_url: String,
    pub container: Option<String>,
    pub mapping: StationMapping,
}

#[derive(Debug, Deserialize)]
pub struct StationMapping {
    pub station_name: String,
    pub station_url: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub tags: Option<String>,
    #[serde(rename = "lastPlaying")]
    pub last_playing: Option<String>,
}

pub fn load_config(
    custom_path: Option<PathBuf>,
    home_dir: Option<PathBuf>,
    current_dir: Option<PathBuf>,
) -> Result<RadioConfig> {
    // Check custom path first
    if let Some(path) = custom_path {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let config: RadioConfig = serde_json::from_str(&content)?;
            return Ok(config);
        } else {
            anyhow::bail!("Custom config file not found: {:?}", path);
        }
    }

    // Check ~/.config/cohors/stations.config.json
    let home = home_dir.or_else(|| std::env::var("HOME").ok().map(PathBuf::from));
    if let Some(h) = home {
        let config_path = h.join(".config/cohors/stations.config.json");
        if config_path.exists() {
            let content = fs::read_to_string(config_path)?;
            let config: RadioConfig = serde_json::from_str(&content)?;
            return Ok(config);
        }
    }

    // Check ./stations.config.json
    let cwd = current_dir.unwrap_or_else(|| PathBuf::from("."));
    let local_path = cwd.join("stations.config.json");
    if local_path.exists() {
        let content = fs::read_to_string(local_path)?;
        let config: RadioConfig = serde_json::from_str(&content)?;
        return Ok(config);
    }

    anyhow::bail!(
        "Config file stations.config.json not found in ~/.config/cohors/ or current directory"
    )
}

pub async fn fetch_all_stations(config_path: Option<PathBuf>) -> Result<Vec<RadioGroup>> {
    let config = load_config(config_path, None, None)?;
    let mut groups = Vec::new();
    for source in config.sources {
        match fetch_stations(&source, None).await {
            Ok(stations) => {
                groups.push(RadioGroup {
                    title: source.title,
                    stations,
                    is_expanded: false,
                });
            }
            Err(e) => eprintln!("Error fetching stations from {}: {}", source.title, e),
        }
    }
    Ok(groups)
}

fn get_cache_path(source_title: &str, home_dir: Option<PathBuf>) -> Option<PathBuf> {
    let home = home_dir.or_else(|| std::env::var("HOME").ok().map(PathBuf::from));
    if let Some(h) = home {
        let cache_dir = h.join(".cache/cohors/stations");
        // Sanitize filename
        let safe_title: String = source_title
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        Some(cache_dir.join(format!("{}.json", safe_title)))
    } else {
        None
    }
}

pub async fn fetch_stations(
    source: &RadioSourceConfig,
    home_dir: Option<PathBuf>,
) -> Result<Vec<RadioStation>> {
    let cache_path = get_cache_path(&source.title, home_dir);
    let mut cached_json: Option<Value> = None;

    if let Some(path) = &cache_path
        && path.exists()
    {
        // Check if file is less than 1 week old
        if let Ok(metadata) = fs::metadata(path)
            && let Ok(modified) = metadata.modified()
        {
            let is_fresh = match SystemTime::now().duration_since(modified) {
                Ok(age) => age < Duration::from_secs(7 * 24 * 60 * 60),
                Err(_) => true, // Time in future means it's fresh
            };

            if is_fresh {
                // Try to read cache
                if let Ok(content) = fs::read_to_string(path)
                    && let Ok(json) = serde_json::from_str(&content)
                {
                    cached_json = Some(json);
                }
            }
        }
    }

    let json = if let Some(json) = cached_json {
        println!("  [CACHE] Loaded '{}'", source.title);
        json
    } else {
        println!("  [WEB] Downloading '{}'...", source.title);
        let response = reqwest::get(&source.json_url).await?;
        let text = response.text().await?;

        // Save to cache
        if let Some(path) = &cache_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(path, &text).is_ok() {
                println!("  [CACHE] Saved '{}'", source.title);
            }
        }

        serde_json::from_str(&text)?
    };

    let items = if let Some(container) = &source.container {
        json.get(container)
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Container not found or not an array"))?
    } else {
        json.as_array()
            .ok_or_else(|| anyhow::anyhow!("Root is not an array"))?
    };

    let mut stations = Vec::new();
    for item in items {
        if let Some(station) = map_station(item, &source.mapping) {
            stations.push(station);
        }
    }
    Ok(stations)
}

fn map_station(item: &Value, mapping: &StationMapping) -> Option<RadioStation> {
    let mut name = get_string_field(item, &mapping.station_name)?
        .trim()
        .to_string();
    let url = get_string_field(item, &mapping.station_url)?
        .trim()
        .to_string();

    if name.is_empty() {
        name = url.clone();
    }

    Some(RadioStation {
        name,
        url,
        description: mapping
            .description
            .as_ref()
            .and_then(|f| get_string_field(item, f))
            .map(|s| s.trim().to_string()),
        homepage: mapping
            .homepage
            .as_ref()
            .and_then(|f| get_string_field(item, f))
            .map(|s| s.trim().to_string()),
        tags: mapping
            .tags
            .as_ref()
            .and_then(|f| get_string_field(item, f))
            .map(|s| s.trim().to_string()),
        last_playing: mapping
            .last_playing
            .as_ref()
            .and_then(|f| get_string_field(item, f))
            .map(|s| s.trim().to_string()),
    })
}

fn get_string_field(item: &Value, path: &str) -> Option<String> {
    let mut current = item;
    for part in path.split('.') {
        if let Ok(index) = part.parse::<usize>() {
            current = current.get(index)?;
        } else {
            current = current.get(part)?;
        }
    }

    match current {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn fetch_pls_stream_url(client: &reqwest::blocking::Client, pls_url: &str) -> Result<String> {
    let response = client.get(pls_url).send()?;

    // Check content type
    if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        let ct = content_type.to_str().unwrap_or("").to_lowercase();
        if ct.contains("audio") || ct.contains("mpeg") || ct.contains("ogg") {
            anyhow::bail!("URL is an audio stream, not a PLS file");
        }
    }

    let content = response.text()?;
    parse_pls(&content)
}

pub fn parse_pls(content: &str) -> Result<String> {
    for line in content.lines() {
        if line.trim().starts_with("File1=") {
            return Ok(line.trim()["File1=".len()..].to_string());
        }
    }
    anyhow::bail!("No stream URL found in PLS")
}

#[cfg(test)]
mod tests {
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
        let stations = fetch_stations(&source, Some(temp_home.path().to_path_buf()))
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
        let stations = fetch_stations(&source, Some(temp_home.path().to_path_buf()))
            .await
            .unwrap();
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].name, "Cached Station");
    }

    #[test]
    fn test_fetch_pls_stream_url_check() {
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

        let client = reqwest::blocking::Client::new();

        // Test audio stream rejection
        let url = format!("{}/stream", server.url());
        let result = fetch_pls_stream_url(&client, &url);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "URL is an audio stream, not a PLS file"
        );

        // Test valid PLS
        let url = format!("{}/playlist.pls", server.url());
        let result = fetch_pls_stream_url(&client, &url);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://stream.com");
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

        let result = fetch_stations(&source, None).await;
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

        let result = fetch_stations(&source, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_fetch_real_stations() {
        // Ensure we can find the config file in the current directory
        let groups = fetch_all_stations(None).await.unwrap();
        println!("Fetched {} groups", groups.len());
        for group in groups {
            println!("Group: {} ({} stations)", group.title, group.stations.len());
        }
    }
}
