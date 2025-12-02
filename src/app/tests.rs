use super::*;
use crate::test_utils::with_xdg_config_home;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_app_initialization() {
    let app = App::new_test();
    assert_eq!(app.volume, 1.0);
    assert!(matches!(app.mode, AppMode::FileSystem));
}

#[test]
fn test_navigation() {
    let mut app = App::new_test();
    app.items = vec![PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")];
    app.update_search_results();
    app.state.select(Some(0));

    app.next();
    assert_eq!(app.state.selected(), Some(1));
    app.next();
    assert_eq!(app.state.selected(), Some(2));
    app.next();
    assert_eq!(app.state.selected(), Some(0)); // Wrap around

    app.previous();
    assert_eq!(app.state.selected(), Some(2)); // Wrap around
    app.previous();
    assert_eq!(app.state.selected(), Some(1));
}

#[test]
fn test_volume_control() {
    let mut app = App::new_test();
    app.volume = 0.5;
    app.change_volume(0.1);
    assert!((app.volume - 0.6).abs() < 0.001);

    app.change_volume(-0.2);
    assert!((app.volume - 0.4).abs() < 0.001);

    app.change_volume(1.0);
    assert_eq!(app.volume, 1.0); // Clamp

    app.change_volume(-2.0);
    assert_eq!(app.volume, 0.0); // Clamp
}

#[test]
fn test_load_directory() {
    let temp = tempdir().unwrap();
    let file1 = temp.path().join("test.mp3");
    let file2 = temp.path().join("test.txt"); // Should be ignored
    let dir1 = temp.path().join("subdir");

    fs::File::create(&file1).unwrap();
    fs::File::create(&file2).unwrap();
    fs::create_dir(&dir1).unwrap();

    let mut app = App::new_test();
    app.current_dir = temp.path().to_path_buf();
    app.load_directory();

    // Items should contain: .. (if not root), subdir, test.mp3

    let names: Vec<String> = app
        .items
        .iter()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "..".to_string())
        })
        .collect();

    assert!(names.contains(&"subdir".to_string()));
    assert!(names.contains(&"test.mp3".to_string()));
    assert!(!names.contains(&"test.txt".to_string()));
}

#[test]
fn test_enter_directory_traversal() {
    let temp = tempdir().unwrap();
    let subdir = temp.path().join("subdir");
    fs::create_dir(&subdir).unwrap();

    let mut app = App::new_test();
    app.current_dir = temp.path().to_path_buf();
    app.load_directory();

    // Find subdir index
    if let Some(idx) = app.items.iter().position(|p| p == &subdir) {
        app.state.select(Some(idx));
        app.enter_directory();
        assert_eq!(app.current_dir, subdir);

        // Now go up
        app.load_directory(); // Reload to see ".."
        if let Some(idx_up) = app.items.iter().position(|p| p.ends_with("..")) {
            app.state.select(Some(idx_up));
            app.enter_directory();
            assert_eq!(app.current_dir, temp.path());
        }
    }
}

#[test]
fn test_toggle_pause() {
    let mut app = App::new_test();
    // Initially not paused
    assert!(!app.is_paused);

    if app.sink.is_some() {
        app.toggle_pause();
        assert!(app.is_paused);
        app.toggle_pause();
        assert!(!app.is_paused);
    }
}

#[test]
fn test_track_switching() {
    let mut app = App::new_test();
    let p1 = PathBuf::from("1.mp3");
    let p2 = PathBuf::from("2.mp3");
    let p3 = PathBuf::from("3.mp3");
    app.items = vec![p1.clone(), p2.clone(), p3.clone()];
    app.update_search_results();

    // Simulate playing p1
    app.current_track = Some(p1.clone());

    // Next track
    app.next_track();
    assert_eq!(app.current_track, Some(p2.clone()));

    // Next track
    app.next_track();
    assert_eq!(app.current_track, Some(p3.clone()));

    app.next_track();
    assert_eq!(app.current_track, Some(p3.clone())); // Stays same if no next track found

    // Previous track
    app.previous_track();
    assert_eq!(app.current_track, Some(p2.clone()));

    app.previous_track();
    assert_eq!(app.current_track, Some(p1.clone()));
}

#[test]
fn test_navigation_none_selected() {
    let mut app = App::new_test();
    app.items = vec![PathBuf::from("a")];
    app.update_search_results();
    app.state.select(None);
    app.next();
    assert_eq!(app.state.selected(), Some(0));

    app.state.select(None);
    app.previous();
    assert_eq!(app.state.selected(), Some(0));
}

#[test]
fn test_load_directory_sorting() {
    let temp = tempdir().unwrap();
    let f1 = temp.path().join("b.mp3");
    let f2 = temp.path().join("a.mp3");
    let d1 = temp.path().join("z_dir");
    let d2 = temp.path().join("a_dir");

    fs::File::create(&f1).unwrap();
    fs::File::create(&f2).unwrap();
    fs::create_dir(&d1).unwrap();
    fs::create_dir(&d2).unwrap();

    let mut app = App::new_test();
    app.current_dir = temp.path().to_path_buf();
    app.load_directory();

    let names: Vec<String> = app
        .items
        .iter()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "..".to_string())
        })
        .collect();

    let filtered: Vec<String> = names.into_iter().filter(|n| n != "..").collect();

    assert_eq!(filtered, vec!["a_dir", "z_dir", "a.mp3", "b.mp3"]);
}

#[test]
fn test_enter_file() {
    let temp = tempdir().unwrap();
    let f1 = temp.path().join("test.mp3");
    fs::File::create(&f1).unwrap();

    let mut app = App::new_test();
    app.items = vec![f1.clone()];
    app.update_search_results();
    app.state.select(Some(0));

    app.enter_directory();

    assert_eq!(app.current_track, Some(f1));
    assert!(!app.is_paused);
}

#[test]
fn test_play_file_with_wav() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("test.wav");

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&file_path, spec).unwrap();
    for t in (0..44100).map(|x| x as f32 / 44100.0) {
        let sample = (t * 440.0 * 2.0 * std::f32::consts::PI).sin();
        let amplitude = i16::MAX as f32;
        writer.write_sample((sample * amplitude) as i16).unwrap();
    }
    writer.finalize().unwrap();

    let mut app = App::new_test();
    app.play_file(file_path.clone());

    assert_eq!(app.current_track, Some(file_path));
    assert!(app.track_duration.is_some());
    assert!(app.playback_start.is_some());
}

struct MockEventSource {
    events: std::collections::VecDeque<Event>,
}

impl MockEventSource {
    fn new(events: Vec<Event>) -> Self {
        Self {
            events: events.into(),
        }
    }
}

impl EventSource for MockEventSource {
    fn poll(&mut self, _timeout: Duration) -> io::Result<bool> {
        Ok(!self.events.is_empty())
    }

    fn read(&mut self) -> io::Result<Event> {
        self.events
            .pop_front()
            .ok_or(io::Error::other("No more events"))
    }
}

#[test]
fn test_run_app() {
    let backend = ratatui::backend::TestBackend::new(100, 50);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = App::new_test();

    // Add some items to navigate
    app.items.push(PathBuf::from("a"));
    app.items.push(PathBuf::from("b"));
    app.update_search_results();
    app.state.select(Some(0));

    let events = vec![
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('j'))), // Next -> 1
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('k'))), // Prev -> 0
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Tab)),       // Mode -> Radio
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Tab)),       // Mode -> Favorites
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Tab)),       // Mode -> FileSystem
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('+'))), // Vol Up
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('-'))), // Vol Down
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Left)),      // Prev Track
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Right)),     // Next Track
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char(' '))), // Pause
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Enter)),     // Enter Dir
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Backspace)), // Go Up
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('?'))), // About
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Esc)),       // Close About
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('h'))), // Hidden
        Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('q'))), // Quit
    ];

    let mut event_source = MockEventSource::new(events);

    let (_tx, rx) = std::sync::mpsc::channel();
    run_app(&mut terminal, &mut app, &mut event_source, &rx).unwrap();

    assert_eq!(app.state.selected(), Some(0));
    assert!(matches!(app.mode, AppMode::FileSystem));
}

#[test]
fn test_on_tick_receives_source() {
    let mut app = App::new_test();
    let (tx, rx) = std::sync::mpsc::channel();
    app.source_receiver = Some(rx);

    // Create a dummy source
    let source = rodio::source::Zero::<f32>::new(1, 44100);
    // Box it
    let boxed_source: Box<dyn Source<Item = f32> + Send> = Box::new(source);

    // Send it
    tx.send(Ok(boxed_source)).unwrap();

    // Call on_tick
    app.on_tick();

    // Verify source_receiver is None (consumed)
    assert!(app.source_receiver.is_none());
}

#[test]
fn test_play_radio_sets_receiver() {
    let mut app = App::new_test();
    let station = crate::radio::RadioStation {
        name: "Test Radio".to_string(),
        url: "http://test.com".to_string(),
        description: Some("Test".to_string()),
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.play_radio(station);
    assert!(app.source_receiver.is_some());
}

#[test]
fn test_radio_navigation() {
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    // Add dummy stations
    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Group 1".to_string(),
        stations: vec![crate::radio::RadioStation {
            name: "1".to_string(),
            url: "1".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        }],
        is_expanded: true,
    });
    app.update_search_results();

    app.radio_state.select(Some(0)); // Group header

    app.next();
    assert_eq!(app.radio_state.selected(), Some(1)); // Station 1

    app.next();
    assert_eq!(app.radio_state.selected(), Some(0)); // Wrap around to Group header

    app.previous();
    assert_eq!(app.radio_state.selected(), Some(1)); // Wrap around to Station 1

    app.previous();
    assert_eq!(app.radio_state.selected(), Some(0));
}

#[test]
fn test_toggle_loop() {
    let mut app = App::new_test();
    assert!(matches!(app.loop_mode, LoopMode::Off));

    app.toggle_loop();
    assert!(matches!(app.loop_mode, LoopMode::Track));

    app.toggle_loop();
    assert!(matches!(app.loop_mode, LoopMode::All));

    app.toggle_loop();
    assert!(matches!(app.loop_mode, LoopMode::Off));
}

#[test]
fn test_next_track_loop_all() {
    let mut app = App::new_test();
    let p1 = PathBuf::from("1.mp3");
    let p2 = PathBuf::from("2.mp3");
    app.items = vec![p1.clone(), p2.clone()];
    app.update_search_results();
    app.loop_mode = LoopMode::All;

    // Play last track
    app.current_track = Some(p2.clone());

    // Next track should wrap to first
    app.next_track();
    // Note: play_file logic might fail if file doesn't exist, but we check if it *tried* to play p1
    // Since play_file sets current_track, we can check that.
    // However, play_file checks if file exists before setting current_track fully?
    // Let's check play_file implementation.
    // play_file sets current_track = Some(path.clone()) at the very beginning.
    assert_eq!(app.current_track, Some(p1));
}

#[test]
fn test_auto_advance_loop_track() {
    let mut app = App::new_test();
    let p1 = PathBuf::from("1.mp3");
    app.items = vec![p1.clone()];
    app.update_search_results();
    app.loop_mode = LoopMode::Track;
    app.current_track = Some(p1.clone());
    app.is_paused = false;

    // Mock sink being empty is hard because Sink::new_idle() returns a sink that is always empty?
    // If sink is empty, on_tick should trigger replay.

    // We need to ensure play_file is called.
    // play_file sets playback_start to Some(Instant::now()).
    app.playback_start = None;

    app.on_tick();

    // If it replayed, playback_start should be set.
    // However, play_file also tries to open the file. If file doesn't exist, it might fail partway.
    // But current_track is set at start of play_file.
    // Let's create a real file for this test.
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("test.mp3");
    fs::File::create(&file_path).unwrap();

    app.items = vec![file_path.clone()];
    app.update_search_results();
    app.current_track = Some(file_path.clone());

    // Reset playback_start to check if it gets updated
    app.playback_start = None;

    app.on_tick();

    assert!(app.playback_start.is_some());
    assert_eq!(app.current_track, Some(file_path));
}

#[test]
fn test_auto_advance_loop_all() {
    let temp = tempdir().unwrap();
    let p1 = temp.path().join("1.mp3");
    let p2 = temp.path().join("2.mp3");
    fs::File::create(&p1).unwrap();
    fs::File::create(&p2).unwrap();

    let mut app = App::new_test();
    app.items = vec![p1.clone(), p2.clone()];
    app.update_search_results();
    app.loop_mode = LoopMode::All;
    app.current_track = Some(p1.clone());
    app.is_paused = false;

    // Sink is empty by default with new_idle()
    app.on_tick();

    // Should have advanced to p2
    assert_eq!(app.current_track, Some(p2));
}

#[test]
fn test_toggle_hidden_files() {
    let temp = tempdir().unwrap();
    let normal_file = temp.path().join("normal.mp3");
    let hidden_file = temp.path().join(".hidden.mp3");

    fs::File::create(&normal_file).unwrap();
    fs::File::create(&hidden_file).unwrap();

    let mut app = App::new_test();
    app.current_dir = temp.path().to_path_buf();

    // Default: Hidden files hidden
    app.load_directory();
    let has_hidden = app
        .items
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("") == ".hidden.mp3");
    assert!(!has_hidden, "Hidden files should not be shown by default");

    // Toggle show_hidden
    app.show_hidden = true;
    app.load_directory();
    let has_hidden = app
        .items
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("") == ".hidden.mp3");
    assert!(
        has_hidden,
        "Hidden files should be shown when show_hidden is true"
    );

    // Toggle back
    app.show_hidden = false;
    app.load_directory();
    let has_hidden = app
        .items
        .iter()
        .any(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("") == ".hidden.mp3");
    assert!(!has_hidden, "Hidden files should be hidden again");
}

#[test]
fn test_http_stream() {
    use std::io::{Read, Seek, SeekFrom};
    let data = b"Hello World";
    let cursor = std::io::Cursor::new(data);
    let mut stream = HttpStream::new(cursor);

    let mut buf = [0u8; 5];
    let n = stream.read(&mut buf).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf, b"Hello");

    // Seek back to start
    let pos = stream.seek(SeekFrom::Start(0)).unwrap();
    assert_eq!(pos, 0);

    // Read again
    let mut buf2 = [0u8; 5];
    let n = stream.read(&mut buf2).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf2, b"Hello");
}

#[test]
fn test_play_radio_no_sink() {
    let mut app = App::new_test();
    app.sink = None;
    let station = crate::radio::RadioStation {
        name: "Test".to_string(),
        url: "http://test.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.play_radio(station);
    assert!(app.source_receiver.is_none());
}

// test_app_new removed due to instability with tarpaulin/ALSA

#[test]
fn test_on_tick_receives_error() {
    let mut app = App::new_test();
    let (tx, rx) = std::sync::mpsc::channel();
    app.source_receiver = Some(rx);

    tx.send(Err("Test Error".to_string())).unwrap();

    app.on_tick();

    assert_eq!(app.last_error, Some("Test Error".to_string()));
    assert!(app.is_paused);
}

#[test]
fn test_play_file_no_sink() {
    let mut app = App::new_test();
    app.sink = None;
    app.play_file(PathBuf::from("test.mp3"));
    assert_eq!(app.last_error, Some("Audio not available".to_string()));
}

#[test]
fn test_play_file_not_found() {
    let mut app = App::new_test();
    app.play_file(PathBuf::from("non_existent.mp3"));
    // Since play_file checks fs::File::open, it should fail silently or log error?
    // The current implementation:
    // if let Ok(file) = fs::File::open(&path) { ... }
    // It doesn't set last_error if file open fails. It just does nothing.
    // But it sets current_track.
    assert_eq!(app.current_track, Some(PathBuf::from("non_existent.mp3")));
    assert!(app.track_duration.is_none());
}

#[test]
fn test_play_radio_thread() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("GET", "/stream")
        .with_status(200)
        .with_body("fake audio data")
        .create();

    let url = format!("{}/stream", server.url());
    let mut app = App::new_test();
    let station = crate::radio::RadioStation {
        name: "Test".to_string(),
        url: url.clone(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.play_radio(station);

    // Wait for thread
    std::thread::sleep(Duration::from_millis(500));

    // Check source_receiver
    if let Some(rx) = &app.source_receiver {
        // It should receive an error because "fake audio data" is not valid audio
        match rx.try_recv() {
            Ok(res) => {
                assert!(res.is_err()); // Decoder error
            }
            Err(_) => {
                // Might still be running or failed silently?
            }
        }
    }
}

#[test]
fn test_favorites() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().to_path_buf();

    with_xdg_config_home(&config_path, || {
        let mut app = App::new_test();
        let path = PathBuf::from("test.mp3");
        app.items.push(path.clone());
        app.state.select(Some(0));

        // Toggle file favorite
        app.toggle_favorite();
        assert!(app.favorites.is_favorite_file(&path));

        // Toggle again to remove
        app.toggle_favorite();
        assert!(!app.favorites.is_favorite_file(&path));

        // Radio favorite
        app.mode = AppMode::Radio;
        let station = crate::radio::RadioStation {
            name: "Test Station".to_string(),
            url: "http://test.com".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        };
        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Test Group".to_string(),
            stations: vec![station.clone()],
            is_expanded: true,
        });
        app.update_search_results();
        app.radio_state.select(Some(1)); // 0 is group header, 1 is station

        app.toggle_favorite();
        assert!(app.favorites.is_favorite_station(&station));

        app.toggle_favorite();
        assert!(!app.favorites.is_favorite_station(&station));
    });
}

#[test]
fn test_on_search_input() {
    let mut app = App::new_test();
    app.items = vec![PathBuf::from("apple"), PathBuf::from("banana")];
    app.update_search_results();

    app.on_search_input('a');
    assert_eq!(app.search_query, "a");
    assert_eq!(app.filtered_items.len(), 2); // apple, banana

    app.on_search_input('p');
    assert_eq!(app.search_query, "ap");
    assert_eq!(app.filtered_items.len(), 1); // apple
}

#[test]
fn test_on_search_backspace() {
    let mut app = App::new_test();
    app.items = vec![PathBuf::from("apple"), PathBuf::from("banana")];
    app.search_query = "ap".to_string();
    app.update_search_results();
    assert_eq!(app.filtered_items.len(), 1);

    app.on_search_backspace();
    assert_eq!(app.search_query, "a");
    assert_eq!(app.filtered_items.len(), 2);
}

#[test]
fn test_cancel_search() {
    let mut app = App::new_test();
    app.is_searching = true;
    app.search_query = "ap".to_string();
    app.update_search_results();

    app.cancel_search();
    assert!(!app.is_searching);
    assert!(app.search_query.is_empty());
    assert_eq!(app.filtered_items.len(), 0); // items is empty in new_test
}

#[test]
fn test_submit_search() {
    let mut app = App::new_test();
    app.is_searching = true;
    app.search_query = "ap".to_string();

    app.submit_search();
    assert!(!app.is_searching);
    assert_eq!(app.search_query, "ap");
}

#[test]
fn test_search_radio() {
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    let station1 = crate::radio::RadioStation {
        name: "Rock FM".to_string(),
        url: "url1".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };
    let station2 = crate::radio::RadioStation {
        name: "Jazz FM".to_string(),
        url: "url2".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Group 1".to_string(),
        stations: vec![station1, station2],
        is_expanded: true,
    });

    app.search_query = "Rock".to_string();
    app.update_search_results();

    assert_eq!(app.filtered_radio_groups.len(), 1);
    assert_eq!(app.filtered_radio_groups[0].stations.len(), 1);
    assert_eq!(app.filtered_radio_groups[0].stations[0].name, "Rock FM");
}

#[test]
fn test_favorites_navigation() {
    let mut app = App::new_test();

    // Setup favorites
    let file_path = PathBuf::from("test_fav.mp3");
    let dir_path = std::env::temp_dir();
    let station = crate::radio::RadioStation {
        name: "Fav Station".to_string(),
        url: "http://fav.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.favorites.files.push(file_path.clone());
    app.favorites.files.push(dir_path.clone());
    app.favorites.stations.push(station.clone());

    app.mode = AppMode::Favorites;

    // 1. Play File
    app.favorites_state.select(Some(0)); // test_fav.mp3
    app.enter_directory();
    assert_eq!(app.current_track, Some(file_path.clone()));

    // 2. Enter Directory
    app.favorites_state.select(Some(1)); // temp dir
    app.enter_directory();
    assert_eq!(app.mode, AppMode::FileSystem);
    assert_eq!(app.current_dir, dir_path);

    // Reset to Favorites
    app.mode = AppMode::Favorites;

    // 3. Play Station
    app.favorites_state.select(Some(2)); // Fav Station
    app.enter_directory();
    assert_eq!(app.current_track, Some(PathBuf::from("Fav Station")));
}

#[test]
fn test_favorites_navigation_wrap() {
    let mut app = App::new_test();
    app.mode = AppMode::Favorites;
    app.favorites.files.push(PathBuf::from("1"));
    app.favorites.files.push(PathBuf::from("2"));

    // Test Next Wrap
    app.favorites_state.select(Some(1));
    app.next();
    assert_eq!(app.favorites_state.selected(), Some(0));

    // Test Previous Wrap
    app.favorites_state.select(Some(0));
    app.previous();
    assert_eq!(app.favorites_state.selected(), Some(1));
}

#[test]
fn test_radio_enter_directory_logic() {
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    let station = crate::radio::RadioStation {
        name: "Station".to_string(),
        url: "http://url".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.radio_groups.push(crate::radio::RadioGroup {
        title: "Group".to_string(),
        stations: vec![station],
        is_expanded: false,
    });
    app.update_search_results();

    // 1. Toggle Group (Expand)
    app.radio_state.select(Some(0));
    app.enter_directory();
    assert!(app.radio_groups[0].is_expanded);

    // 2. Play Station
    app.radio_state.select(Some(1)); // Station is now visible at index 1
    app.enter_directory();
    assert_eq!(app.current_track, Some(PathBuf::from("Station")));

    // 3. Toggle Group (Collapse)
    app.radio_state.select(Some(0));
    app.enter_directory();
    assert!(!app.radio_groups[0].is_expanded);
}

#[test]
fn test_play_radio_http_error() {
    let mut server = mockito::Server::new();
    let _m = server.mock("GET", "/stream").with_status(404).create();

    let url = format!("{}/stream", server.url());
    let mut app = App::new_test();
    let station = crate::radio::RadioStation {
        name: "Test".to_string(),
        url,
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.play_radio(station);
    std::thread::sleep(Duration::from_millis(500));

    if let Some(rx) = &app.source_receiver {
        match rx.try_recv() {
            Ok(Err(msg)) => assert!(msg.contains("HTTP error")),
            _ => panic!("Expected HTTP error"),
        }
    }
}

#[test]
fn test_play_radio_connection_error() {
    // Use a port that is likely closed or a non-routable IP to force connection error
    let url = "http://127.0.0.1:54321/stream".to_string();
    let mut app = App::new_test();
    let station = crate::radio::RadioStation {
        name: "Test".to_string(),
        url,
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    app.play_radio(station);
    std::thread::sleep(Duration::from_millis(500));

    if let Some(rx) = &app.source_receiver {
        match rx.try_recv() {
            Ok(Err(msg)) => assert!(msg.contains("Connection error")),
            _ => panic!("Expected Connection error"),
        }
    }
}

#[test]
fn test_mpris_commands() {
    let mut app = App::new_test();

    // Test Volume
    app.handle_mpris_command(MprisCommand::Volume(0.5));
    assert_eq!(app.volume, 0.5);

    // Test Play/Pause
    app.is_paused = false;
    app.handle_mpris_command(MprisCommand::Pause);
    assert!(app.is_paused);

    app.handle_mpris_command(MprisCommand::Play);
    assert!(!app.is_paused);

    app.handle_mpris_command(MprisCommand::PlayPause);
    assert!(app.is_paused);

    // Test Stop (pauses for now)
    app.is_paused = false;
    app.handle_mpris_command(MprisCommand::Stop);
    assert!(app.is_paused);

    // Test Next/Prev (mock items)
    app.items = vec![PathBuf::from("a"), PathBuf::from("b")];
    app.update_search_results();
    app.state.select(Some(0));
    app.current_track = Some(PathBuf::from("a"));

    app.handle_mpris_command(MprisCommand::Next);
    assert_eq!(app.state.selected(), Some(1));

    app.handle_mpris_command(MprisCommand::Previous);
    assert_eq!(app.state.selected(), Some(0));

    // Test Seek
    app.is_paused = true;
    app.playback_start = None;
    app.playback_elapsed = Duration::from_secs(10);
    app.handle_mpris_command(MprisCommand::Seek(5_000_000)); // +5s
    assert_eq!(app.playback_elapsed, Duration::from_secs(15));

    app.handle_mpris_command(MprisCommand::Seek(-2_000_000)); // -2s
    assert_eq!(app.playback_elapsed, Duration::from_secs(13));

    // Test SetPosition
    app.handle_mpris_command(MprisCommand::SetPosition("id".to_string(), 30_000_000)); // 30s
    assert_eq!(app.playback_elapsed, Duration::from_secs(30));

    // Test LoopStatus
    app.handle_mpris_command(MprisCommand::LoopStatus(mpris_server::LoopStatus::Track));
    assert!(matches!(app.loop_mode, LoopMode::Track));

    app.handle_mpris_command(MprisCommand::LoopStatus(mpris_server::LoopStatus::Playlist));
    assert!(matches!(app.loop_mode, LoopMode::All));

    app.handle_mpris_command(MprisCommand::LoopStatus(mpris_server::LoopStatus::None));
    assert!(matches!(app.loop_mode, LoopMode::Off));
}

#[test]
fn test_search_in_modes() {
    let mut app = App::new_test();

    // Radio Mode
    app.mode = AppMode::Radio;
    app.on_search_input('a');
    assert_eq!(app.search_query, "a");
    // Check if selection reset
    assert_eq!(app.radio_state.selected(), Some(0));

    app.on_search_backspace();
    assert_eq!(app.search_query, "");
    assert_eq!(app.radio_state.selected(), Some(0));

    app.on_search_input('b');
    app.cancel_search();
    assert_eq!(app.search_query, "");
    assert!(!app.is_searching);
    assert_eq!(app.radio_state.selected(), Some(0));

    // Favorites Mode
    app.mode = AppMode::Favorites;
    app.on_search_input('a');
    assert_eq!(app.search_query, "a");
    assert_eq!(app.favorites_state.selected(), Some(0));

    app.on_search_backspace();
    assert_eq!(app.search_query, "");
    assert_eq!(app.favorites_state.selected(), Some(0));

    app.on_search_input('b');
    app.cancel_search();
    assert_eq!(app.search_query, "");
    assert!(!app.is_searching);
    assert_eq!(app.favorites_state.selected(), Some(0));
}

#[test]
fn test_radio_indexing() {
    let mut app = App::new_test();
    let station = RadioStation {
        name: "Test".to_string(),
        url: "http://test.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };
    let group1 = RadioGroup {
        title: "Group1".to_string(),
        stations: vec![station.clone()],
        is_expanded: false,
    };
    let group2 = RadioGroup {
        title: "Group2".to_string(),
        stations: vec![station.clone()],
        is_expanded: true,
    };
    app.radio_groups.push(group1);
    app.radio_groups.push(group2);
    app.update_search_results();

    // Index 0: Group 1 Header
    assert!(app.get_radio_station_at_index(0).is_none());

    // Index 1: Group 2 Header
    assert!(app.get_radio_station_at_index(1).is_none());

    // Index 2: Group 2 Station 1
    let s = app.get_radio_station_at_index(2);
    assert!(s.is_some());
    assert_eq!(s.unwrap().name, "Test");

    // Index 3: Out of bounds
    assert!(app.get_radio_station_at_index(3).is_none());
}

#[test]
fn test_check_for_updates_integration() {
    let mut app = App::new_test();
    let (tx, rx) = std::sync::mpsc::channel();
    app.update_receiver = Some(rx);

    // Simulate update found
    tx.send(Some("9.9.9".to_string())).unwrap();
    app.on_tick();
    assert_eq!(app.latest_version, Some("9.9.9".to_string()));
    assert!(app.update_receiver.is_none());

    // Simulate no update
    let mut app = App::new_test();
    let (tx, rx) = std::sync::mpsc::channel();
    app.update_receiver = Some(rx);
    tx.send(None).unwrap();
    app.on_tick();
    assert_eq!(app.latest_version, None);
    assert!(app.update_receiver.is_none());
}

#[test]
fn test_get_selected_station() {
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    let station = crate::radio::RadioStation {
        name: "Test Station".to_string(),
        url: "http://test.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    let group = crate::radio::RadioGroup {
        title: "Test Group".to_string(),
        stations: vec![station.clone()],
        is_expanded: true,
    };

    app.radio_groups.push(group);
    app.update_search_results();

    // Select group header
    app.radio_state.select(Some(0));
    assert!(app.get_selected_station().is_none());

    // Select station
    app.radio_state.select(Some(1));
    let selected = app.get_selected_station();
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().name, "Test Station");
}

#[test]
fn test_save_radio_station() {
    let mut app = App::new_test();
    app.mode = AppMode::Radio;

    let temp = tempfile::TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");
    app.config_path = Some(config_path.clone());

    let station = crate::radio::RadioStation {
        name: "Test Station".to_string(),
        url: "http://test.com".to_string(),
        description: None,
        homepage: None,
        tags: None,
        last_playing: None,
    };

    let group = crate::radio::RadioGroup {
        title: "Test Group".to_string(),
        stations: vec![station.clone()],
        is_expanded: true,
    };

    app.radio_groups.push(group);
    app.update_search_results();

    // Select station
    app.radio_state.select(Some(1));

    app.save_radio_station();

    assert!(config_path.exists());
    assert!(app.notification.is_some());
    assert!(app.notification.unwrap().0.contains("Exported"));
}

#[test]
fn test_save_radio_station_wrong_mode() {
    let mut app = App::new_test();
    app.mode = AppMode::FileSystem;

    let temp = tempfile::TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");
    app.config_path = Some(config_path.clone());

    app.save_radio_station();

    assert!(!config_path.exists());
    assert!(app.notification.is_none());
}

#[test]
fn test_notification_expiry() {
    let mut app = App::new_test();
    app.notification = Some((
        "Test".to_string(),
        std::time::Instant::now() - std::time::Duration::from_secs(4),
    ));

    app.on_tick();

    assert!(app.notification.is_none());
}

#[test]
fn test_add_modal_flow() {
    let mut app = App::new_test();
    let temp = tempfile::TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");
    app.config_path = Some(config_path.clone());

    // Open modal
    app.open_add_modal();
    assert!(matches!(
        app.add_modal_state,
        Some(AddModalState::Selection)
    ));

    // Select Station
    app.handle_add_modal_input(KeyCode::Char('s'));
    if let Some(AddModalState::InputStation {
        name,
        focused_field,
        ..
    }) = &app.add_modal_state
    {
        assert_eq!(name, "");
        assert_eq!(*focused_field, 0);
    } else {
        panic!("Expected InputStation state");
    }

    // Type Name
    app.handle_add_modal_input(KeyCode::Char('T'));
    app.handle_add_modal_input(KeyCode::Char('e'));
    app.handle_add_modal_input(KeyCode::Char('s'));
    app.handle_add_modal_input(KeyCode::Char('t'));

    // Next field (URL)
    app.handle_add_modal_input(KeyCode::Tab);

    // Type URL
    app.handle_add_modal_input(KeyCode::Char('h'));
    app.handle_add_modal_input(KeyCode::Char('t'));
    app.handle_add_modal_input(KeyCode::Char('t'));
    app.handle_add_modal_input(KeyCode::Char('p'));

    // Save
    app.handle_add_modal_input(KeyCode::Enter);

    // Check if saved
    assert!(app.add_modal_state.is_none());
    assert!(config_path.exists());
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("Test"));
    assert!(content.contains("http"));
}

#[test]
fn test_add_modal_source_flow() {
    let mut app = App::new_test();
    let temp = tempfile::TempDir::new().unwrap();
    let config_path = temp.path().join("stations.config.json");
    app.config_path = Some(config_path.clone());

    // Open modal
    app.open_add_modal();

    // Select Source
    app.handle_add_modal_input(KeyCode::Char('r'));
    if let Some(AddModalState::InputSource {
        title,
        focused_field,
        ..
    }) = &app.add_modal_state
    {
        assert_eq!(title, "");
        assert_eq!(*focused_field, 0);
    } else {
        panic!("Expected InputSource state");
    }

    // Type Title
    app.handle_add_modal_input(KeyCode::Char('S'));
    app.handle_add_modal_input(KeyCode::Char('r'));
    app.handle_add_modal_input(KeyCode::Char('c'));

    // Next field (JSON URL)
    app.handle_add_modal_input(KeyCode::Tab);
    app.handle_add_modal_input(KeyCode::Char('h'));
    app.handle_add_modal_input(KeyCode::Char('t'));
    app.handle_add_modal_input(KeyCode::Char('t'));
    app.handle_add_modal_input(KeyCode::Char('p'));

    // Next field (Container) - Optional
    app.handle_add_modal_input(KeyCode::Tab);

    // Next field (Map Name)
    app.handle_add_modal_input(KeyCode::Tab);
    app.handle_add_modal_input(KeyCode::Char('n'));

    // Next field (Map URL)
    app.handle_add_modal_input(KeyCode::Tab);
    app.handle_add_modal_input(KeyCode::Char('u'));

    // Save
    app.handle_add_modal_input(KeyCode::Enter);

    // Check if saved
    assert!(app.add_modal_state.is_none());
    assert!(config_path.exists());
    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("Src"));
}

#[test]
fn test_add_modal_navigation() {
    let mut app = App::new_test();
    app.open_add_modal();
    app.handle_add_modal_input(KeyCode::Char('s')); // Station mode

    // Check initial focus
    if let Some(AddModalState::InputStation { focused_field, .. }) = &app.add_modal_state {
        assert_eq!(*focused_field, 0);
    }

    // Tab forward
    app.handle_add_modal_input(KeyCode::Tab);
    if let Some(AddModalState::InputStation { focused_field, .. }) = &app.add_modal_state {
        assert_eq!(*focused_field, 1);
    }

    // BackTab backward
    app.handle_add_modal_input(KeyCode::BackTab);
    if let Some(AddModalState::InputStation { focused_field, .. }) = &app.add_modal_state {
        assert_eq!(*focused_field, 0);
    }

    // Wrap around backward
    app.handle_add_modal_input(KeyCode::BackTab);
    if let Some(AddModalState::InputStation { focused_field, .. }) = &app.add_modal_state {
        assert_eq!(*focused_field, 4); // Last field
    }
}

#[test]
fn test_add_modal_cancel() {
    let mut app = App::new_test();
    app.open_add_modal();
    assert!(app.add_modal_state.is_some());

    app.handle_add_modal_input(KeyCode::Esc);
    assert!(app.add_modal_state.is_none());

    // Cancel from input state
    app.open_add_modal();
    app.handle_add_modal_input(KeyCode::Char('s'));
    assert!(matches!(
        app.add_modal_state,
        Some(AddModalState::InputStation { .. })
    ));

    app.handle_add_modal_input(KeyCode::Esc);
    assert!(app.add_modal_state.is_none());
}

#[test]
fn test_add_modal_validation() {
    let mut app = App::new_test();
    app.open_add_modal();

    // 1. Station Validation
    app.handle_add_modal_input(KeyCode::Char('s'));
    // Try to save empty
    app.handle_add_modal_input(KeyCode::Enter);

    assert!(app.notification.is_some());
    assert!(app.notification.as_ref().unwrap().0.contains("required"));
    // Should still be in modal
    assert!(matches!(
        app.add_modal_state,
        Some(AddModalState::InputStation { .. })
    ));

    // 2. Source Validation
    app.add_modal_state = None;
    app.notification = None;
    app.open_add_modal();
    app.handle_add_modal_input(KeyCode::Char('r'));

    // Try to save empty
    app.handle_add_modal_input(KeyCode::Enter);

    assert!(app.notification.is_some());
    assert!(app.notification.as_ref().unwrap().0.contains("required"));
    // Should still be in modal
    assert!(matches!(
        app.add_modal_state,
        Some(AddModalState::InputSource { .. })
    ));
}

#[test]
fn test_update_mpris_state() {
    let mut app = App::new_test();
    let mpris_state =
        std::sync::Arc::new(std::sync::Mutex::new(crate::mpris::MprisState::default()));
    app.mpris_state = Some(mpris_state.clone());

    app.volume = 0.8;
    app.loop_mode = LoopMode::Track;
    app.current_track = Some(PathBuf::from("test_song.mp3"));
    app.track_duration = Some(Duration::from_secs(180));

    app.update_mpris();

    let state = mpris_state.lock().unwrap();
    // Use a larger epsilon for f32 to f64 conversion
    assert!((state.volume - 0.8).abs() < 1e-6);
    assert!(matches!(state.loop_status, mpris_server::LoopStatus::Track));
    assert_eq!(state.title, "test_song.mp3");
    assert_eq!(state.duration, Some(Duration::from_secs(180)));
}
