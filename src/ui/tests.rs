use super::*;
use crate::app::{App, AppMode};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn test_ui_draw_notification() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.notification = Some(("Test Notification".to_string(), Instant::now()));

    terminal.draw(|f| draw(f, &mut app)).unwrap();

    let buffer = terminal.backend().buffer();
    // Check if notification text is present in the buffer
    let mut found = false;
    for cell in buffer.content.iter() {
        if cell.symbol() == "T" {
            // Start of "Test Notification"
            found = true;
            break;
        }
    }
    assert!(found, "Notification text not found in buffer");
}

#[test]
fn test_ui_draw_file_list() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();

    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("subdir");
    let file = temp.path().join("song.mp3");
    std::fs::create_dir(&dir).unwrap();
    std::fs::File::create(&file).unwrap();

    app.items = vec![dir.clone(), file.clone()];
    app.state.select(Some(0));

    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.state.select(Some(1));
    app.current_track = Some(file.clone());

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_playing_no_duration() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.current_track = Some(PathBuf::from("stream.mp3"));
    app.track_duration = None;

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_about() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.show_help = true;

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_error() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.last_error = Some("Test Error".to_string());

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_playing() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.current_track = Some(PathBuf::from("test.mp3"));
    app.is_paused = false;
    app.track_duration = Some(Duration::from_secs(100));
    app.playback_elapsed = Duration::from_secs(10);
    app.playback_start = Some(Instant::now());

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_radio() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Test Group".to_string(),
        stations: vec![crate::radio::RadioStation {
            name: "Test Station".to_string(),
            url: "http://test.com".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        }],
        is_expanded: true,
    });
    app.update_search_results();
    app.radio_state.select(Some(0));

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_radio_large_list() {
    let backend = TestBackend::new(100, 20); // Small height to force scrolling
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    let mut stations = Vec::new();
    for i in 0..50 {
        stations.push(crate::radio::RadioStation {
            name: format!("Station {}", i),
            url: "http://test.com".to_string(),
            description: Some("Desc".to_string()),
            homepage: None,
            tags: None,
            last_playing: None,
        });
    }

    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Large Group".to_string(),
        stations,
        is_expanded: true,
    });
    app.update_search_results();

    // Select an item that requires scrolling (e.g., index 30)
    // Index 0 is group header, stations start at 1.
    // So station 30 is at index 31.
    app.radio_state.select(Some(31));

    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // Verify offset was updated
    assert!(app.radio_state.offset() > 0);
}

#[test]
fn test_ui_draw_radio_scrolling_optimization() {
    let backend = TestBackend::new(100, 10); // Very short height
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    // Group 1: 5 stations (Index 0-5)
    let mut stations1 = Vec::new();
    for i in 0..5 {
        stations1.push(crate::radio::RadioStation {
            name: format!("Station 1-{}", i),
            url: "u".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        });
    }
    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Group 1".to_string(),
        stations: stations1,
        is_expanded: true,
    });

    // Group 2: 5 stations (Index 6-11)
    let mut stations2 = Vec::new();
    for i in 0..5 {
        stations2.push(crate::radio::RadioStation {
            name: format!("Station 2-{}", i),
            url: "u".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        });
    }
    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Group 2".to_string(),
        stations: stations2,
        is_expanded: true,
    });
    app.update_search_results();

    // Select something in Group 2 to force scrolling Group 1 out of view
    // List height is ~10 (minus borders/titles/etc).
    // If we select index 10 (Station 2-4), offset should be around 10-height+1.
    app.radio_state.select(Some(10));

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_visualizer_empty_data() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.current_track = Some(PathBuf::from("test.mp3"));
    app.is_paused = false;
    // Force empty spectrum data
    *app.spectrum_data.lock().unwrap() = Vec::new();

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_radio_scroll_up() {
    let backend = TestBackend::new(100, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    // Add enough items
    let mut stations = Vec::new();
    for i in 0..20 {
        stations.push(crate::radio::RadioStation {
            name: format!("Station {}", i),
            url: "u".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        });
    }
    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Group".to_string(),
        stations,
        is_expanded: true,
    });
    app.update_search_results();

    // Set offset to 10, selected to 5
    app.radio_state = app.radio_state.clone().with_offset(10);
    app.radio_state.select(Some(5));

    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // Offset should become 5
    assert_eq!(app.radio_state.offset(), 5);
}

#[test]
fn test_ui_draw_search_mode() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.is_searching = true;
    app.search_query = "test".to_string();

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_search_filter_active() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.is_searching = false;
    app.search_query = "filter".to_string();

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_loading() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    // Simulate loading state by setting source_receiver
    let (_tx, rx) = std::sync::mpsc::channel();
    app.source_receiver = Some(rx);

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_loop_modes() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.current_track = Some(PathBuf::from("test.mp3"));

    app.loop_mode = crate::app::LoopMode::Track;
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.loop_mode = crate::app::LoopMode::All;
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_current_track_highlight() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    let track = PathBuf::from("test.mp3");
    app.items = vec![track.clone()];
    app.update_search_results();
    app.current_track = Some(track);

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_visualizer_no_bars() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();

    // Clear spectrum data
    if let Ok(mut data) = app.spectrum_data.lock() {
        data.clear();
    }

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_favorite_file() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();

    let file = PathBuf::from("fav.mp3");
    app.items = vec![file.clone()];
    app.update_search_results();
    app.favorites.files.push(file.clone());

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_invalid_selection() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();

    app.items = vec![PathBuf::from("a")];
    app.update_search_results();
    app.state.select(Some(10)); // Invalid index

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_favorites_coverage() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Favorites;

    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("fav_dir");
    let file = temp.path().join("fav_file.mp3");
    std::fs::create_dir(&dir).unwrap();
    std::fs::File::create(&file).unwrap();

    app.favorites.files.push(dir);
    app.favorites.files.push(file);

    app.favorites.stations.push(crate::radio::RadioStation {
        name: "Fav Station".to_string(),
        url: "http://fav.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    });

    // Case 1: Select Directory (to cover is_dir branch)
    app.favorites_state.select(Some(0));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // Case 2: Select File
    app.favorites_state.select(Some(1));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // Case 3: Select Station
    app.favorites_state.select(Some(2));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // Case 4: Out of bounds selection
    app.favorites_state.select(Some(99));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // Case 5: No selection
    app.favorites_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_version_update() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();

    // Set update available
    app.latest_version = Some("9.9.9".to_string());

    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_add_modal() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();

    // 1. Selection State
    app.add_modal_state = Some(AddModalState::Selection);
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // 2. Input Station State
    app.add_modal_state = Some(AddModalState::InputStation {
        name: "Test Station".to_string(),
        url: "http://test.com".to_string(),
        description: "Desc".to_string(),
        homepage: "Home".to_string(),
        tags: "Tags".to_string(),
        focused_field: 0,
        original_url: None,
    });
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // Test focus on different fields
    if let Some(AddModalState::InputStation { focused_field, .. }) = &mut app.add_modal_state {
        *focused_field = 1;
    }
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    // 3. Input Source State
    for i in 0..8 {
        app.add_modal_state = Some(AddModalState::InputSource {
            title: "Test Source".to_string(),
            json_url: "http://json.com".to_string(),
            container: "data".to_string(),
            map_name: "n".to_string(),
            map_url: "u".to_string(),
            map_desc: "d".to_string(),
            map_home: "h".to_string(),
            map_tags: "t".to_string(),
            focused_field: i,
            original_title: None,
        });
        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    // 4. Input Subsonic State
    for i in 0..3 {
        app.add_modal_state = Some(AddModalState::InputSubsonic {
            server_url: "http://nav.com".to_string(),
            username: "u".to_string(),
            password: "p".to_string(),
            focused_field: i,
            original_url: None,
        });
        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    // 5. Edit Subsonic State
    app.add_modal_state = Some(AddModalState::InputSubsonic {
        server_url: "http://nav.com".to_string(),
        username: "u".to_string(),
        password: "p".to_string(),
        focused_field: 0,
        original_url: Some("http://nav.com".to_string()),
    });
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_subsonic_artists() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Subsonic;
    app.subsonic_view = crate::app::SubsonicView::Artists;

    app.subsonic_clients
        .push(crate::subsonic::SubsonicClient::new(
            crate::config::SubsonicSourceConfig {
                server_url: "http://navi.test".to_string(),
                username: "u".to_string(),
                password: Some("p".to_string()),
                auth_token: None,
            },
        ));
    app.active_subsonic_client = 0;

    app.subsonic_artists.push(crate::subsonic::Artist {
        id: "1".to_string(),
        name: "Test Artist".to_string(),
        album_count: Some(5),
    });

    app.subsonic_state.select(Some(0));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.subsonic_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_subsonic_albums() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Subsonic;
    app.subsonic_view = crate::app::SubsonicView::Albums("1".to_string());

    app.subsonic_clients
        .push(crate::subsonic::SubsonicClient::new(
            crate::config::SubsonicSourceConfig {
                server_url: "http://navi.test".to_string(),
                username: "u".to_string(),
                password: Some("p".to_string()),
                auth_token: None,
            },
        ));

    app.subsonic_albums.push(crate::subsonic::Album {
        id: "2".to_string(),
        name: "Test Album".to_string(),
        artist: Some("Test Artist".to_string()),
        artist_id: Some("1".to_string()),
        song_count: Some(10),
        duration: Some(1200),
        year: Some(2025),
    });

    app.subsonic_state.select(Some(0));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.subsonic_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_subsonic_tracks() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::Subsonic;
    app.subsonic_view = crate::app::SubsonicView::Tracks("2".to_string());

    app.subsonic_clients
        .push(crate::subsonic::SubsonicClient::new(
            crate::config::SubsonicSourceConfig {
                server_url: "http://navi.test".to_string(),
                username: "u".to_string(),
                password: Some("p".to_string()),
                auth_token: None,
            },
        ));

    app.subsonic_tracks.push(crate::subsonic::Track {
        id: "3".to_string(),
        parent: Some("2".to_string()),
        is_dir: false,
        title: "Test Track".to_string(),
        album: Some("Test Album".to_string()),
        artist: Some("Test Artist".to_string()),
        track: Some(1),
        duration: Some(180),
        size: Some(10485760), // 10 MB
    });

    app.subsonic_state.select(Some(0));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.subsonic_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

// ─── AudioBookshelf UI tests ──────────────────────────────────────────────────

fn make_abs_client() -> crate::audiobookshelf::AudioBookshelfClient {
    crate::audiobookshelf::AudioBookshelfClient::new(crate::config::AbsSourceConfig {
        server_url: "http://abs.test:13378".to_string(),
        username: "user".to_string(),
        api_token: "token".to_string(),
    })
}

fn make_abs_ep(id: &str, finished: bool, current_time: f64) -> crate::audiobookshelf::AbsEpisode {
    crate::audiobookshelf::AbsEpisode {
        id: id.to_string(),
        library_item_id: "pod1".to_string(),
        title: format!("Episode {id}"),
        description: Some("A great episode.".to_string()),
        published_at: Some(946684800000), // 2000-01-01
        duration: Some(3600.0),
        is_finished: finished,
        current_time,
    }
}

#[test]
fn test_ui_draw_abs_servers() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::AudioBookshelf;
    app.abs_view = crate::app::AbsView::Servers;
    app.abs_clients.push(make_abs_client());
    app.abs_state.select(Some(0));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.abs_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_abs_libraries() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::AudioBookshelf;
    app.abs_view = crate::app::AbsView::Libraries;
    app.abs_clients.push(make_abs_client());
    app.abs_libraries.push(crate::audiobookshelf::AbsLibrary {
        id: "lib1".to_string(),
        name: "Podcasts".to_string(),
        media_type: "podcast".to_string(),
    });
    app.abs_state.select(Some(0));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.abs_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_abs_podcasts() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::AudioBookshelf;
    app.abs_view = crate::app::AbsView::Podcasts("lib1".to_string());
    app.abs_clients.push(make_abs_client());
    app.abs_podcasts.push(crate::audiobookshelf::AbsPodcast {
        id: "pod1".to_string(),
        media: crate::audiobookshelf::AbsPodcastMedia {
            metadata: crate::audiobookshelf::AbsPodcastMetadata {
                title: "My Podcast".to_string(),
                author: Some("Jane Doe".to_string()),
                description: None,
            },
            episodes: vec![],
            num_episodes: Some(42),
        },
    });
    app.abs_state.select(Some(0));
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.abs_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_abs_episodes() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::AudioBookshelf;
    app.abs_view = crate::app::AbsView::Episodes("pod1".to_string());
    app.abs_clients.push(make_abs_client());
    app.abs_episodes = vec![
        make_abs_ep("ep1", false, 0.0),    // unplayed
        make_abs_ep("ep2", false, 600.0),  // in progress
        make_abs_ep("ep3", true, 3600.0),  // finished
    ];
    // Iterate each row to exercise all info-panel branches
    for i in 0..3 {
        app.abs_state.select(Some(i));
        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }
    app.abs_state.select(None);
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}

#[test]
fn test_ui_draw_abs_episodes_filter_sort_hints() {
    let backend = TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();
    app.mode = AppMode::AudioBookshelf;
    app.abs_view = crate::app::AbsView::Episodes("pod1".to_string());
    app.abs_clients.push(make_abs_client());
    app.abs_episodes = vec![make_abs_ep("ep1", false, 0.0)];
    app.abs_state.select(Some(0));

    app.abs_hide_played = true;
    app.abs_sort_oldest_first = true;
    terminal.draw(|f| draw(f, &mut app)).unwrap();

    app.abs_hide_played = false;
    app.abs_sort_oldest_first = false;
    terminal.draw(|f| draw(f, &mut app)).unwrap();
}
