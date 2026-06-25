use crate::audiobookshelf::{
    AbsEpisode, AbsPodcast, AbsPodcastMedia, AbsPodcastMetadata, AudioBookshelfClient,
};
use crate::config::AbsSourceConfig;
use mockito::Server;

fn make_config(url: &str) -> AbsSourceConfig {
    AbsSourceConfig {
        server_url: url.to_string(),
        username: "testuser".to_string(),
        api_token: "testtoken".to_string(),
    }
}

fn make_client(url: &str) -> AudioBookshelfClient {
    AudioBookshelfClient::new(make_config(url))
}

fn bare_episode(id: &str) -> AbsEpisode {
    AbsEpisode {
        id: id.to_string(),
        library_item_id: String::new(),
        title: format!("Episode {id}"),
        description: None,
        published_at: None,
        duration: None,
        is_finished: false,
        current_time: 0.0,
    }
}

fn bare_podcast(id: &str, title: &str, author: Option<&str>) -> AbsPodcast {
    AbsPodcast {
        id: id.to_string(),
        media: AbsPodcastMedia {
            metadata: AbsPodcastMetadata {
                title: title.to_string(),
                author: author.map(str::to_string),
                description: None,
            },
            episodes: Vec::new(),
            num_episodes: None,
        },
    }
}

// --- URL / auth helpers ---

#[test]
fn test_build_url_no_trailing_slash() {
    let client = make_client("http://abs.local:13378");
    assert_eq!(
        client.build_url("/api/libraries"),
        "http://abs.local:13378/api/libraries"
    );
}

#[test]
fn test_build_url_strips_trailing_slash() {
    let client = make_client("http://abs.local:13378/");
    assert_eq!(
        client.build_url("/api/libraries"),
        "http://abs.local:13378/api/libraries"
    );
}

#[test]
fn test_auth_header() {
    let client = make_client("http://abs.local");
    assert_eq!(client.auth_header(), "Bearer testtoken");
}

// --- AbsEpisode helpers ---

#[test]
fn test_episode_published_date_valid() {
    let mut ep = bare_episode("e1");
    ep.published_at = Some(946684800000); // 2000-01-01 00:00:00 UTC
    assert_eq!(ep.published_date(), "2000-01-01");
}

#[test]
fn test_episode_published_date_none() {
    assert_eq!(bare_episode("e1").published_date(), "Unknown");
}

#[test]
fn test_episode_published_date_zero() {
    let mut ep = bare_episode("e1");
    ep.published_at = Some(0);
    assert_eq!(ep.published_date(), "Unknown");
}

#[test]
fn test_episode_duration_str_valid() {
    let mut ep = bare_episode("e1");
    ep.duration = Some(3661.0); // 61 min 1 sec
    assert_eq!(ep.duration_str(), "61:01");
}

#[test]
fn test_episode_duration_str_whole_hours() {
    let mut ep = bare_episode("e1");
    ep.duration = Some(3600.0);
    assert_eq!(ep.duration_str(), "60:00");
}

#[test]
fn test_episode_duration_str_none() {
    assert_eq!(bare_episode("e1").duration_str(), "--:--");
}

#[test]
fn test_episode_duration_str_zero() {
    let mut ep = bare_episode("e1");
    ep.duration = Some(0.0);
    assert_eq!(ep.duration_str(), "--:--");
}

// --- AbsPodcast helpers ---

#[test]
fn test_podcast_title() {
    assert_eq!(bare_podcast("p1", "My Show", None).title(), "My Show");
}

#[test]
fn test_podcast_author_some() {
    assert_eq!(
        bare_podcast("p1", "My Show", Some("Jane Doe")).author(),
        Some("Jane Doe")
    );
}

#[test]
fn test_podcast_author_none() {
    assert_eq!(bare_podcast("p1", "My Show", None).author(), None);
}

#[test]
fn test_podcast_num_episodes_some() {
    let mut p = bare_podcast("p1", "My Show", None);
    p.media.num_episodes = Some(42);
    assert_eq!(p.num_episodes(), 42);
}

#[test]
fn test_podcast_num_episodes_none() {
    assert_eq!(bare_podcast("p1", "My Show", None).num_episodes(), 0);
}

// --- HTTP: login ---

#[tokio::test]
async fn test_login_success() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/login")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"user": {"token": "my-api-token"}}"#)
        .create_async()
        .await;

    let result = AudioBookshelfClient::login(&server.url(), "user", "pass").await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "my-api-token");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_login_bad_credentials() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("POST", "/login")
        .with_status(401)
        .with_body("Unauthorized")
        .create_async()
        .await;

    let result = AudioBookshelfClient::login(&server.url(), "user", "wrong").await;
    assert!(result.is_err());
    mock.assert_async().await;
}

// --- HTTP: get_podcast_libraries ---

#[tokio::test]
async fn test_get_podcast_libraries_filters_non_podcast() {
    let mut server = Server::new_async().await;
    let body = r#"{
        "libraries": [
            {"id": "lib1", "name": "Podcasts", "mediaType": "podcast"},
            {"id": "lib2", "name": "Audiobooks", "mediaType": "book"}
        ]
    }"#;
    let mock = server
        .mock("GET", "/api/libraries")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_podcast_libraries()
        .await
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "lib1");
    assert_eq!(result[0].name, "Podcasts");
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_podcast_libraries_empty_when_none_are_podcasts() {
    let mut server = Server::new_async().await;
    let body = r#"{"libraries": [{"id": "lib1", "name": "Books", "mediaType": "book"}]}"#;
    server
        .mock("GET", "/api/libraries")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_podcast_libraries()
        .await
        .unwrap();
    assert!(result.is_empty());
}

// --- HTTP: get_podcasts ---

#[tokio::test]
async fn test_get_podcasts() {
    let mut server = Server::new_async().await;
    let body = r#"{
        "results": [
            {
                "id": "pod1",
                "media": {
                    "metadata": {"title": "My Show", "author": "Jane", "description": null},
                    "numEpisodes": 5
                }
            },
            {
                "id": "pod2",
                "media": {
                    "metadata": {"title": "Another Show", "author": null, "description": null},
                    "numEpisodes": 10
                }
            }
        ]
    }"#;
    let mock = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/api/libraries/lib1/items.*".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_podcasts("lib1")
        .await
        .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, "pod1");
    assert_eq!(result[0].title(), "My Show");
    assert_eq!(result[0].num_episodes(), 5);
    assert_eq!(result[1].id, "pod2");
    assert_eq!(result[1].author(), None);
    mock.assert_async().await;
}

// --- HTTP: get_episodes (with progress merge) ---

#[tokio::test]
async fn test_get_episodes_merges_progress() {
    let mut server = Server::new_async().await;
    let item_body = r#"{
        "id": "pod1",
        "media": {
            "metadata": {"title": "My Show", "author": null, "description": null},
            "episodes": [
                {
                    "id": "ep1", "title": "Episode 1", "description": null,
                    "publishedAt": 946684800000, "duration": 3600.0
                },
                {
                    "id": "ep2", "title": "Episode 2", "description": null,
                    "publishedAt": 946771200000, "duration": 1800.0
                }
            ]
        }
    }"#;
    let me_body = r#"{
        "mediaProgress": [
            {
                "libraryItemId": "pod1", "episodeId": "ep1",
                "isFinished": true, "currentTime": 3590.0, "duration": 3600.0
            }
        ]
    }"#;

    let item_mock = server
        .mock("GET", "/api/items/pod1?include=progress")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(item_body)
        .create_async()
        .await;
    let me_mock = server
        .mock("GET", "/api/me")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(me_body)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_episodes("pod1")
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
    // ep1 should have progress merged in
    assert_eq!(result[0].id, "ep1");
    assert!(result[0].is_finished);
    assert_eq!(result[0].current_time, 3590.0);
    assert_eq!(result[0].library_item_id, "pod1");
    // ep2 has no progress entry — defaults remain
    assert_eq!(result[1].id, "ep2");
    assert!(!result[1].is_finished);
    assert_eq!(result[1].current_time, 0.0);

    item_mock.assert_async().await;
    me_mock.assert_async().await;
}

#[tokio::test]
async fn test_get_episodes_fills_duration_from_progress() {
    let mut server = Server::new_async().await;
    // Episode has no duration field in JSON
    let item_body = r#"{
        "id": "pod1",
        "media": {
            "metadata": {"title": "My Show", "author": null, "description": null},
            "episodes": [{"id": "ep1", "title": "E1", "description": null, "publishedAt": null}]
        }
    }"#;
    let me_body = r#"{
        "mediaProgress": [
            {
                "libraryItemId": "pod1", "episodeId": "ep1",
                "isFinished": false, "currentTime": 300.0, "duration": 1800.0
            }
        ]
    }"#;

    server
        .mock("GET", "/api/items/pod1?include=progress")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(item_body)
        .create_async()
        .await;
    server
        .mock("GET", "/api/me")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(me_body)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_episodes("pod1")
        .await
        .unwrap();
    assert_eq!(result[0].duration, Some(1800.0));
    assert_eq!(result[0].current_time, 300.0);
}

// --- HTTP: get_stream_url ---

#[tokio::test]
async fn test_get_stream_url_absolute_content_url() {
    let mut server = Server::new_async().await;
    let body = r#"{"audioTracks": [{"contentUrl": "http://cdn.example.com/ep1.mp3"}]}"#;
    server
        .mock("POST", "/api/items/pod1/play/ep1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let url = make_client(&server.url())
        .get_stream_url("pod1", "ep1", 0.0)
        .await
        .unwrap();
    assert_eq!(url, "http://cdn.example.com/ep1.mp3?token=testtoken");
}

#[tokio::test]
async fn test_get_stream_url_relative_content_url() {
    let mut server = Server::new_async().await;
    let body = r#"{"audioTracks": [{"contentUrl": "/s/item/pod1/ep1.mp3"}]}"#;
    server
        .mock("POST", "/api/items/pod1/play/ep1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let url = make_client(&server.url())
        .get_stream_url("pod1", "ep1", 0.0)
        .await
        .unwrap();
    assert!(url.starts_with(&server.url()), "should be prefixed with server URL");
    assert!(url.contains("/s/item/pod1/ep1.mp3"));
    assert!(url.ends_with("?token=testtoken"));
}

#[tokio::test]
async fn test_get_stream_url_no_tracks_returns_error() {
    let mut server = Server::new_async().await;
    server
        .mock("POST", "/api/items/pod1/play/ep1")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"audioTracks": []}"#)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_stream_url("pod1", "ep1", 0.0)
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("no audio tracks"));
}

// --- HTTP: update_progress ---

#[tokio::test]
async fn test_update_progress() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("PATCH", "/api/me/progress/pod1/ep1")
        .with_status(200)
        .with_body("")
        .create_async()
        .await;

    let result = make_client(&server.url())
        .update_progress("pod1", "ep1", 90.0, 100.0)
        .await;
    assert!(result.is_ok());
    mock.assert_async().await;
}

// --- HTTP: get_user_progress ---

#[tokio::test]
async fn test_get_user_progress() {
    let mut server = Server::new_async().await;
    let body = r#"{
        "mediaProgress": [
            {
                "libraryItemId": "pod1", "episodeId": "ep1",
                "isFinished": false, "currentTime": 120.0, "duration": 3600.0
            }
        ]
    }"#;
    let mock = server
        .mock("GET", "/api/me")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_user_progress()
        .await
        .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].library_item_id, "pod1");
    assert_eq!(result[0].episode_id, Some("ep1".to_string()));
    assert!(!result[0].is_finished);
    assert_eq!(result[0].current_time, 120.0);
    assert_eq!(result[0].duration, 3600.0);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_user_progress_empty() {
    let mut server = Server::new_async().await;
    server
        .mock("GET", "/api/me")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"mediaProgress": []}"#)
        .create_async()
        .await;

    let result = make_client(&server.url())
        .get_user_progress()
        .await
        .unwrap();
    assert!(result.is_empty());
}
