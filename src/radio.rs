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
    let text = response.text().await?;
    parse_channels(&text)
}

pub fn parse_channels(json: &str) -> Result<Vec<Channel>> {
    let channel_response: ChannelResponse = serde_json::from_str(json)?;
    Ok(channel_response.channels)
}

pub fn fetch_pls_stream_url(pls_url: &str) -> Result<String> {
    let content = reqwest::blocking::get(pls_url)?.text()?;
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

    #[test]
    fn test_parse_pls() {
        let pls_content = "[playlist]\nNumberOfEntries=1\nFile1=http://ice1.somafm.com/groovesalad-128-mp3\nTitle1=SomaFM: Groove Salad\nLength1=-1\nVersion=2";
        let url = parse_pls(pls_content).unwrap();
        assert_eq!(url, "http://ice1.somafm.com/groovesalad-128-mp3");
    }

    #[test]
    fn test_parse_pls_failure() {
        let pls_content = "[playlist]\nNumberOfEntries=0\nVersion=2";
        assert!(parse_pls(pls_content).is_err());
    }

    #[test]
    fn test_parse_channels() {
        let json = r#"{
            "channels": [
                {
                    "id": "groovesalad",
                    "title": "Groove Salad",
                    "description": "A nicely chilled plate of ambient/downtempo beats and grooves.",
                    "dj": "Rusty",
                    "genre": "ambient",
                    "image": "http://somafm.com/img/groovesalad120.png",
                    "listeners": "1500",
                    "playlists": [
                        { "url": "http://somafm.com/groovesalad.pls", "format": "mp3", "quality": "highest" }
                    ]
                }
            ]
        }"#;
        let channels = parse_channels(json).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].id, "groovesalad");
        assert_eq!(channels[0].title, "Groove Salad");
        assert_eq!(channels[0].playlists.len(), 1);
    }
}

