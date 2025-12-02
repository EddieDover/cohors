use crate::config::{
    AppConfig, IndividualStationConfig, RadioConfig, RadioSourceConfig, StationMapping,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct RadioStation {
    pub name: String,
    pub url: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub tags: Option<String>,
    pub last_playing: Option<String>,
}

pub fn load_config() -> Result<RadioConfig> {
    // Otherwise load AppConfig
    let app_config = AppConfig::load()?;
    Ok(app_config.radio)
}

pub async fn fetch_all_stations(
    home_dir: Option<PathBuf>,
    invalidate_cache: bool,
    silent: bool,
) -> Result<Vec<RadioGroup>> {
    let config = load_config()?;
    let mut groups = Vec::new();

    // Process individual stations first
    if !config.individual_stations.is_empty() {
        let mut stations = Vec::new();
        for s in config.individual_stations {
            stations.push(RadioStation {
                name: s.name,
                url: s.station_url,
                description: s.description,
                homepage: s.homepage,
                tags: s.tags,
                last_playing: None,
            });
        }
        groups.push(RadioGroup {
            title: "Custom Stations".to_string(),
            stations,
            is_expanded: true,
        });
    }

    for source in config.sources {
        match fetch_stations(&source, home_dir.clone(), invalidate_cache, silent).await {
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

pub fn invalidate_source_cache(title: &str, home_dir: Option<PathBuf>) {
    if let Some(path) = get_cache_path(title, home_dir)
        && path.exists()
    {
        let _ = std::fs::remove_file(path);
    }
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
    invalidate_cache: bool,
    silent: bool,
) -> Result<Vec<RadioStation>> {
    let cache_path = get_cache_path(&source.title, home_dir);
    let mut cached_json: Option<Value> = None;

    if invalidate_cache {
        if let Some(path) = &cache_path
            && path.exists()
        {
            let _ = fs::remove_file(path);
            if !silent {
                println!("  [CACHE] Invalidated '{}'", source.title);
            }
        }
    } else if let Some(path) = &cache_path
        && path.exists()
    {
        // Verify file age is less than 1 week
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
        if !silent {
            println!("  [CACHE] Loaded '{}'", source.title);
        }
        json
    } else {
        if !silent {
            println!("  [WEB] Downloading '{}'...", source.title);
        }
        let response = reqwest::get(&source.json_url).await?;
        let text = response.text().await?;

        // Save to cache
        if let Some(path) = &cache_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(path, &text).is_ok() && !silent {
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

pub fn fetch_playlist_stream_url(client: &reqwest::blocking::Client, url: &str) -> Result<String> {
    let response = client.get(url).send()?;

    // Check content type
    if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        let ct = content_type.to_str().unwrap_or("").to_lowercase();
        // Allow audio/x-scpls (PLS files) and mpegurl (M3U) but reject other audio streams
        let is_playlist = ct.contains("scpls") || ct.contains("mpegurl") || ct.contains("m3u");
        if (ct.contains("audio") || ct.contains("mpeg") || ct.contains("ogg")) && !is_playlist {
            anyhow::bail!("URL is an audio stream, not a playlist file");
        }
    }

    let content = response.text()?;

    // Try parsing as PLS
    if let Ok(url) = parse_pls(&content) {
        return Ok(url);
    }

    // Try parsing as M3U
    parse_m3u(&content)
}

pub fn parse_pls(content: &str) -> Result<String> {
    for line in content.lines() {
        if line.trim().starts_with("File1=") {
            return Ok(line.trim()["File1=".len()..].to_string());
        }
    }
    anyhow::bail!("No stream URL found in PLS")
}

pub fn parse_m3u(content: &str) -> Result<String> {
    for line in content.lines() {
        let line = line.trim();
        if !line.is_empty() && !line.starts_with('#') {
            return Ok(line.to_string());
        }
    }
    anyhow::bail!("No stream URL found in M3U")
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_export;

pub fn add_station_to_config(station: &RadioStation) -> Result<()> {
    let mut config = AppConfig::load()?;

    // Prevent duplicates
    if config
        .radio
        .individual_stations
        .iter()
        .any(|s| s.station_url == station.url)
    {
        return Ok(()); // Already exists
    }

    config
        .radio
        .individual_stations
        .push(IndividualStationConfig {
            name: station.name.clone(),
            station_url: station.url.clone(),
            description: station.description.clone(),
            homepage: station.homepage.clone(),
            tags: station.tags.clone(),
        });

    config.save()?;
    Ok(())
}

pub fn add_source_to_config(source: &RadioSourceConfig) -> Result<()> {
    let mut config = AppConfig::load()?;

    // Prevent duplicates
    if config
        .radio
        .sources
        .iter()
        .any(|s| s.title == source.title || s.json_url == source.json_url)
    {
        return Ok(()); // Already exists
    }

    config.radio.sources.push(source.clone());

    config.save()?;
    Ok(())
}

pub fn edit_station_in_config(old_url: &str, station: &RadioStation) -> Result<()> {
    let mut config = AppConfig::load()?;

    if let Some(idx) = config
        .radio
        .individual_stations
        .iter()
        .position(|s| s.station_url == old_url)
    {
        config.radio.individual_stations[idx] = IndividualStationConfig {
            name: station.name.clone(),
            station_url: station.url.clone(),
            description: station.description.clone(),
            homepage: station.homepage.clone(),
            tags: station.tags.clone(),
        };
        config.save()?;
    }
    Ok(())
}

pub fn edit_source_in_config(old_title: &str, source: &RadioSourceConfig) -> Result<()> {
    let mut config = AppConfig::load()?;

    if let Some(idx) = config
        .radio
        .sources
        .iter()
        .position(|s| s.title == old_title)
    {
        config.radio.sources[idx] = source.clone();
        config.save()?;
    }
    Ok(())
}
