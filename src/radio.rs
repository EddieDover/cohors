use serde::Deserialize;
use anyhow::Result;

// Attribution: Data provided by SomaFM (https://somafm.com).
// Please support them!

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Channel {
    pub id: String,
    pub title: String,
    pub description: String,
    pub dj: String,
    pub genre: String,
    pub image: Option<String>,
    pub listeners: String,
    pub playlists: Vec<Playlist>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Playlist {
    pub url: String,
    pub format: String,
    pub quality: String,
}

#[derive(Debug, Deserialize)]
struct ChannelResponse {
    channels: Vec<Channel>,
}

pub async fn fetch_channels() -> Result<Vec<Channel>> {
    let url = "https://somafm.com/channels.json";
    let response = reqwest::get(url).await?;
    let channel_response: ChannelResponse = response.json().await?;
    Ok(channel_response.channels)
}

pub fn fetch_pls_stream_url(pls_url: &str) -> Result<String> {
    let content = reqwest::blocking::get(pls_url)?.text()?;
    for line in content.lines() {
        if line.trim().starts_with("File1=") {
            return Ok(line.trim()["File1=".len()..].to_string());
        }
    }
    anyhow::bail!("No stream URL found in PLS")
}

