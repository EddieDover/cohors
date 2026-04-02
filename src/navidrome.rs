#![allow(dead_code)]

use crate::config::NavidromeSourceConfig;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct SubsonicClient {
    client: Client,
    pub config: NavidromeSourceConfig,
    client_name: String,
    version: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponse {
    #[serde(rename = "subsonic-response")]
    pub subsonic_response: SubsonicResponseBody,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponseBody {
    pub status: String,
    pub version: String,
    pub artists: Option<ArtistsContainer>,
    pub artist: Option<ArtistWithAlbums>,
    pub album: Option<AlbumWithTracks>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArtistWithAlbums {
    pub id: String,
    pub name: String,
    pub album: Option<Vec<Album>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AlbumWithTracks {
    pub id: String,
    pub name: String,
    pub song: Option<Vec<Track>>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub artist: Option<String>,
    pub artist_id: Option<String>,
    pub song_count: Option<u32>,
    pub duration: Option<u32>,
    pub year: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub parent: Option<String>,
    pub is_dir: bool,
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub track: Option<u32>,
    pub duration: Option<u32>,
    pub size: Option<u64>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArtistsContainer {
    pub index: Vec<ArtistIndex>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArtistIndex {
    pub name: String,
    pub artist: Vec<Artist>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub album_count: Option<u32>,
}

impl SubsonicClient {
    pub fn new(config: NavidromeSourceConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            client_name: "cohors".to_string(),
            version: "1.16.1".to_string(),
        }
    }

    fn generate_auth_params(&self) -> Vec<(&str, String)> {
        let mut params = vec![
            ("u", self.config.username.clone()),
            ("v", self.version.clone()),
            ("c", self.client_name.clone()),
            ("f", "json".to_string()),
        ];

        // Token auth
        use rand::distr::SampleString;
        let salt: String = rand::distr::Alphanumeric.sample_string(&mut rand::rng(), 6);
        let password = self.config.password.clone().unwrap_or_default();
        let payload = format!("{}{}", password, salt);
        let token = format!("{:x}", md5::compute(payload));

        params.push(("t", token));
        params.push(("s", salt));

        params
    }

    fn build_url(&self, endpoint: &str) -> String {
        let base_url = self.config.server_url.trim_end_matches('/');
        format!("{}/rest/{}", base_url, endpoint)
    }

    pub async fn ping(&self) -> Result<()> {
        let url = self.build_url("ping");
        let params = self.generate_auth_params();

        let resp: SubsonicResponse = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        if resp.subsonic_response.status == "ok" {
            Ok(())
        } else {
            anyhow::bail!("Subsonic API ping failed")
        }
    }

    pub async fn get_artists(&self) -> Result<Vec<Artist>> {
        let url = self.build_url("getArtists");
        let params = self.generate_auth_params();

        let resp: SubsonicResponse = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        let mut all_artists = Vec::new();
        if let Some(artists) = resp.subsonic_response.artists {
            for index in artists.index {
                all_artists.extend(index.artist);
            }
        }

        Ok(all_artists)
    }

    pub async fn get_artist(&self, id: &str) -> Result<Vec<Album>> {
        let url = self.build_url("getArtist");
        let mut params = self.generate_auth_params();
        params.push(("id", id.to_string()));

        let resp: SubsonicResponse = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp
            .subsonic_response
            .artist
            .and_then(|a| a.album)
            .unwrap_or_default())
    }

    pub async fn get_album(&self, id: &str) -> Result<Vec<Track>> {
        let url = self.build_url("getAlbum");
        let mut params = self.generate_auth_params();
        params.push(("id", id.to_string()));

        let resp: SubsonicResponse = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp
            .subsonic_response
            .album
            .and_then(|a| a.song)
            .unwrap_or_default())
    }

    pub fn get_stream_url(&self, track_id: &str) -> String {
        let url = self.build_url("stream");
        let mut params = self.generate_auth_params();
        params.push(("id", track_id.to_string()));

        let query: Vec<String> = params
            .into_iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(&v)))
            .collect();

        format!("{}?{}", url, query.join("&"))
    }
}

#[cfg(test)]
mod tests;

