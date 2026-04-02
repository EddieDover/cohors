use crate::config::NavidromeSourceConfig;
use crate::navidrome::SubsonicClient;
use mockito::Server;

fn baseline_config(url: &str) -> NavidromeSourceConfig {
    NavidromeSourceConfig {
        username: "testuser".to_string(),
        password: Some("testpass".to_string()),
        server_url: url.to_string(),
        auth_token: None,
    }
}

#[test]
fn test_build_url() {
    let config = baseline_config("http://localhost:4533");
    let client = SubsonicClient::new(config);
    assert_eq!(client.build_url("ping"), "http://localhost:4533/rest/ping");

    let config2 = baseline_config("http://localhost:4533/");
    let client2 = SubsonicClient::new(config2);
    assert_eq!(client2.build_url("ping"), "http://localhost:4533/rest/ping");
}

#[test]
fn test_generate_auth_params() {
    let config = baseline_config("http://localhost:4533");
    let client = SubsonicClient::new(config);
    let auth_params = client.generate_auth_params();

    // The params should contain specific items
    let u = auth_params.iter().find(|(k, _)| *k == "u").unwrap();
    assert_eq!(u.1, "testuser");

    let f = auth_params.iter().find(|(k, _)| *k == "f").unwrap();
    assert_eq!(f.1, "json");

    let t = auth_params.iter().find(|(k, _)| *k == "t");
    let s = auth_params.iter().find(|(k, _)| *k == "s");
    assert!(t.is_some());
    assert!(s.is_some());

    let t_val = &t.unwrap().1;
    let s_val = &s.unwrap().1;

    let payload = format!("testpass{}", s_val);
    let expected_token = format!("{:x}", md5::compute(payload));
    assert_eq!(t_val, &expected_token);
}

#[tokio::test]
async fn test_ping_success() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", mockito::Matcher::Regex(r"^/rest/ping.*".to_string()))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"subsonic-response": {"status": "ok", "version": "1.16.1"}}"#)
        .create_async()
        .await;

    let config = baseline_config(&server.url());
    let client = SubsonicClient::new(config);

    let result = client.ping().await;
    assert!(result.is_ok());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_ping_failure() {
    let mut server = Server::new_async().await;
    let mock = server
        .mock("GET", mockito::Matcher::Regex(r"^/rest/ping.*".to_string()))
        .with_status(200) // status 200 but failed response payload
        .with_header("content-type", "application/json")
        .with_body(r#"{"subsonic-response": {"status": "failed", "version": "1.16.1"}}"#)
        .create_async()
        .await;

    let config = baseline_config(&server.url());
    let client = SubsonicClient::new(config);

    let result = client.ping().await;
    assert!(result.is_err());
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_artists() {
    let mut server = Server::new_async().await;
    let body = r#"
    {
       "subsonic-response": {
          "status": "ok",
          "version": "1.16.1",
          "artists": {
             "index": [
                {
                   "name": "A",
                   "artist": [
                      {
                         "id": "1",
                         "name": "Artist 1",
                         "albumCount": 2
                      }
                   ]
                }
             ]
          }
       }
    }"#;
    let mock = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/rest/getArtists.*".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let config = baseline_config(&server.url());
    let client = SubsonicClient::new(config);

    let result = client.get_artists().await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "1");
    assert_eq!(result[0].name, "Artist 1");
    assert_eq!(result[0].album_count, Some(2));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_artist() {
    let mut server = Server::new_async().await;
    let body = r#"
    {
       "subsonic-response": {
          "status": "ok",
          "version": "1.16.1",
          "artist": {
             "id": "1",
             "name": "Artist 1",
             "album": [
                {
                   "id": "100",
                   "name": "Album 100",
                   "artist": "Artist 1",
                   "duration": 1800
                }
             ]
          }
       }
    }"#;
    let mock = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/rest/getArtist.*id=1.*".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let config = baseline_config(&server.url());
    let client = SubsonicClient::new(config);

    let result = client.get_artist("1").await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "100");
    assert_eq!(result[0].name, "Album 100");
    assert_eq!(result[0].duration, Some(1800));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_get_album() {
    let mut server = Server::new_async().await;
    let body = r#"
    {
       "subsonic-response": {
          "status": "ok",
          "version": "1.16.1",
          "album": {
             "id": "100",
             "name": "Album 100",
             "song": [
                {
                   "id": "1000",
                   "isDir": false,
                   "title": "Track 1",
                   "album": "Album 100",
                   "artist": "Artist 1",
                   "track": 1,
                   "duration": 200
                }
             ]
          }
       }
    }"#;
    let mock = server
        .mock(
            "GET",
            mockito::Matcher::Regex(r"^/rest/getAlbum.*id=100.*".to_string()),
        )
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(body)
        .create_async()
        .await;

    let config = baseline_config(&server.url());
    let client = SubsonicClient::new(config);

    let result = client.get_album("100").await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, "1000");
    assert_eq!(result[0].title, "Track 1");
    assert_eq!(result[0].duration, Some(200));
    assert!(!result[0].is_dir);
    mock.assert_async().await;
}

#[test]
fn test_get_stream_url() {
    let config = baseline_config("http://localhost:4533");
    let client = SubsonicClient::new(config);

    let stream_url = client.get_stream_url("my_track_123");
    assert!(stream_url.starts_with("http://localhost:4533/rest/stream?"));
    assert!(stream_url.contains("id=my_track_123"));
    assert!(stream_url.contains("u=testuser"));
    assert!(stream_url.contains("c=cohors"));
    assert!(stream_url.contains("v=1.16.1"));
}
