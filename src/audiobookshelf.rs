#![allow(dead_code)]

use crate::config::AbsSourceConfig;
use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AudioBookshelfClient {
    client: Client,
    pub config: AbsSourceConfig,
}

// --- API response types ---

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AbsLibrary {
    pub id: String,
    pub name: String,
    pub media_type: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AbsPodcast {
    pub id: String,
    pub media: AbsPodcastMedia,
}

impl AbsPodcast {
    pub fn title(&self) -> &str {
        &self.media.metadata.title
    }

    pub fn author(&self) -> Option<&str> {
        self.media.metadata.author.as_deref()
    }

    pub fn num_episodes(&self) -> u32 {
        self.media.num_episodes.unwrap_or(0)
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AbsPodcastMedia {
    pub metadata: AbsPodcastMetadata,
    #[serde(default)]
    pub episodes: Vec<AbsEpisode>,
    pub num_episodes: Option<u32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AbsPodcastMetadata {
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AbsEpisode {
    pub id: String,
    /// Set after fetch — not present in JSON (comes from the parent item ID).
    #[serde(skip, default)]
    pub library_item_id: String,
    pub title: String,
    pub description: Option<String>,
    /// Milliseconds since Unix epoch.
    pub published_at: Option<i64>,
    pub duration: Option<f64>,
    /// Populated after merging with user progress.
    #[serde(skip, default)]
    pub is_finished: bool,
    /// Populated after merging with user progress.
    #[serde(skip, default)]
    pub current_time: f64,
}

impl AbsEpisode {
    pub fn published_date(&self) -> String {
        match self.published_at {
            Some(ts) if ts > 0 => ts_ms_to_date(ts),
            _ => "Unknown".to_string(),
        }
    }

    pub fn duration_str(&self) -> String {
        match self.duration {
            Some(d) if d > 0.0 => {
                let secs = d as u64;
                format!("{}:{:02}", secs / 60, secs % 60)
            }
            _ => "--:--".to_string(),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AbsMediaProgress {
    pub library_item_id: String,
    pub episode_id: Option<String>,
    pub is_finished: bool,
    pub current_time: f64,
    pub duration: f64,
}

// --- Internal response wrappers ---

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LibrariesResponse {
    libraries: Vec<AbsLibrary>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LibraryItemsResponse {
    results: Vec<AbsPodcast>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PlaybackSession {
    audio_tracks: Vec<AudioTrack>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AudioTrack {
    content_url: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    user: LoginUser,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LoginUser {
    token: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct MeResponse {
    #[serde(default)]
    media_progress: Vec<AbsMediaProgress>,
}

// --- Client impl ---

impl AudioBookshelfClient {
    pub fn new(config: AbsSourceConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.config.api_token)
    }

    fn build_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.config.server_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    /// Log in and return the user's API token.
    pub async fn login(server_url: &str, username: &str, password: &str) -> Result<String> {
        let client = Client::new();
        let url = format!("{}/login", server_url.trim_end_matches('/'));
        let body = serde_json::json!({ "username": username, "password": password });
        let resp: LoginResponse = client.post(&url).json(&body).send().await?.json().await?;
        Ok(resp.user.token)
    }

    /// Returns only libraries of type "podcast".
    pub async fn get_podcast_libraries(&self) -> Result<Vec<AbsLibrary>> {
        let url = self.build_url("/api/libraries");
        let resp: LibrariesResponse = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp
            .libraries
            .into_iter()
            .filter(|l| l.media_type == "podcast")
            .collect())
    }

    /// Returns all podcast items in a library.
    pub async fn get_podcasts(&self, library_id: &str) -> Result<Vec<AbsPodcast>> {
        let url = self.build_url(&format!(
            "/api/libraries/{}/items?limit=1000&sort=media.metadata.title&desc=0",
            library_id
        ));
        let resp: LibraryItemsResponse = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.results)
    }

    /// Returns all episodes for a podcast item, with progress merged in.
    pub async fn get_episodes(&self, library_item_id: &str) -> Result<Vec<AbsEpisode>> {
        let url = self.build_url(&format!("/api/items/{}?include=progress", library_item_id));
        let item: AbsPodcast = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?
            .json()
            .await?;

        let progress = self.get_user_progress().await.unwrap_or_default();
        let mut episodes = item.media.episodes;
        for ep in &mut episodes {
            ep.library_item_id = library_item_id.to_string();
            if let Some(prog) = progress.iter().find(|p| {
                p.library_item_id == library_item_id && p.episode_id.as_deref() == Some(&ep.id)
            }) {
                ep.is_finished = prog.is_finished;
                ep.current_time = prog.current_time;
                if ep.duration.is_none() && prog.duration > 0.0 {
                    ep.duration = Some(prog.duration);
                }
            }
        }
        Ok(episodes)
    }

    /// Starts a direct-play session and returns the stream URL (with token query param).
    pub async fn get_stream_url(
        &self,
        library_item_id: &str,
        episode_id: &str,
        start_offset: f64,
    ) -> Result<String> {
        let url = self.build_url(&format!(
            "/api/items/{}/play/{}",
            library_item_id, episode_id
        ));
        let body = serde_json::json!({
            "deviceInfo": { "clientName": "cohors", "deviceId": "cohors-tui" },
            "forceDirectPlay": true,
            "forceTranscode": false,
            "startOffset": start_offset,
        });
        let resp: PlaybackSession = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        let track = resp
            .audio_tracks
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("ABS returned no audio tracks"))?;

        let full_url = if track.content_url.starts_with("http") {
            track.content_url
        } else {
            format!(
                "{}{}",
                self.config.server_url.trim_end_matches('/'),
                track.content_url
            )
        };

        Ok(format!("{}?token={}", full_url, self.config.api_token))
    }

    /// Reports playback progress back to ABS.
    pub async fn update_progress(
        &self,
        library_item_id: &str,
        episode_id: &str,
        current_time: f64,
        duration: f64,
    ) -> Result<()> {
        let url = self.build_url(&format!(
            "/api/me/progress/{}/{}",
            library_item_id, episode_id
        ));
        let progress = if duration > 0.0 {
            current_time / duration
        } else {
            0.0
        };
        let is_finished = progress > 0.95;
        let body = serde_json::json!({
            "currentTime": current_time,
            "duration": duration,
            "progress": progress,
            "isFinished": is_finished,
        });
        self.client
            .patch(&url)
            .header("Authorization", self.auth_header())
            .json(&body)
            .send()
            .await?;
        Ok(())
    }

    pub async fn get_user_progress(&self) -> Result<Vec<AbsMediaProgress>> {
        let url = self.build_url("/api/me");
        let resp: MeResponse = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?
            .json()
            .await?;
        Ok(resp.media_progress)
    }
}

#[cfg(test)]
mod tests;

// --- Utility ---

/// Converts a Unix timestamp in milliseconds to a YYYY-MM-DD string.
fn ts_ms_to_date(ts_ms: i64) -> String {
    let z = ts_ms / 1000 / 86400 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}
