use std::{fs, io, path::PathBuf, time::{Duration, Instant}, sync::{Arc, Mutex}};
use ratatui::widgets::ListState;
use rodio::{Decoder, OutputStream, Sink, OutputStreamHandle, Source};
use crate::audio::AudioAnalyzer;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{backend::Backend, Terminal};
use crate::ui;

pub enum AppMode {
    FileSystem,
    Radio,
}

pub struct App {
    pub mode: AppMode,
    pub current_dir: PathBuf,
    pub items: Vec<PathBuf>,
    pub state: ListState,
    // Audio
    pub _stream: Option<OutputStream>,
    pub _stream_handle: Option<OutputStreamHandle>,
    pub sink: Option<Sink>,
    pub volume: f32,
    pub current_track: Option<PathBuf>,
    pub is_paused: bool,
    pub last_error: Option<String>,
    // Playback state
    pub track_duration: Option<Duration>,
    pub playback_start: Option<Instant>,
    pub playback_elapsed: Duration,
    // Visualizer
    pub spectrum_data: Arc<Mutex<Vec<(&'static str, u64)>>>,
}

impl App {
    pub fn new() -> App {
        let (stream, stream_handle, sink, error) = match OutputStream::try_default() {
            Ok((s, h)) => match Sink::try_new(&h) {
                Ok(sink) => (Some(s), Some(h), Some(sink), None),
                Err(e) => (Some(s), Some(h), None, Some(format!("Sink error: {}", e))),
            },
            Err(e) => (None, None, None, Some(format!("Audio init error: {}", e))),
        };

        let volume = 1.0;
        if let Some(s) = &sink {
            s.set_volume(volume);
        }

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut app = App {
            mode: AppMode::FileSystem,
            current_dir,
            items: Vec::new(),
            state: ListState::default(),
            _stream: stream,
            _stream_handle: stream_handle,
            sink,
            volume,
            current_track: None,
            is_paused: false,
            last_error: error,
            track_duration: None,
            playback_start: None,
            playback_elapsed: Duration::from_secs(0),
            spectrum_data: Arc::new(Mutex::new(vec![("", 0); 8])),
        };
        app.load_directory();
        app
    }

    #[cfg(test)]
    pub fn new_test() -> App {
        let (sink, _queue) = Sink::new_idle();
        App {
            mode: AppMode::FileSystem,
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            items: Vec::new(),
            state: ListState::default(),
            _stream: None,
            _stream_handle: None,
            sink: Some(sink),
            volume: 1.0,
            current_track: None,
            is_paused: false,
            last_error: None,
            track_duration: None,
            playback_start: None,
            playback_elapsed: Duration::from_secs(0),
            spectrum_data: Arc::new(Mutex::new(vec![("", 0); 8])),
        }
    }

    pub fn load_directory(&mut self) {
        self.items.clear();

        // Add parent directory option if not at root
        if self.current_dir.parent().is_some() {
            self.items.push(self.current_dir.join(".."));
        }

        let mut entries_vec = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    entries_vec.push(path);
                } else if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_str.as_str(), "mp3" | "wav" | "ogg" | "flac") {
                        entries_vec.push(path);
                    }
                }
            }
        }
        // Sort directories first, then files
        entries_vec.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir && !b_is_dir {
                std::cmp::Ordering::Less
            } else if !a_is_dir && b_is_dir {
                std::cmp::Ordering::Greater
            } else {
                a.file_name().cmp(&b.file_name())
            }
        });

        self.items.extend(entries_vec);
        self.state.select(Some(0));
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn enter_directory(&mut self) {
        if let Some(path) = self.state.selected().and_then(|i| self.items.get(i)) {
            if path.ends_with("..") {
                self.go_up();
            } else if path.is_dir() {
                self.current_dir = path.clone();
                self.load_directory();
            } else {
                // Play music
                self.play_file(path.clone());
            }
        }
    }

    pub fn play_file(&mut self, path: PathBuf) {
        self.current_track = Some(path.clone());
        self.is_paused = false;
        self.playback_start = Some(Instant::now());
        self.playback_elapsed = Duration::from_secs(0);
        self.track_duration = None;

        if let Some(sink) = &self.sink {
            if let Ok(file) = fs::File::open(&path) {
                let reader = io::BufReader::new(file);
                if let Ok(decoder) = Decoder::new(reader) {
                    self.track_duration = decoder.total_duration();
                    
                    // Convert to f32 for analysis
                    let source = decoder.convert_samples::<f32>();
                    let sample_rate = source.sample_rate();
                    
                    // Wrap in analyzer
                    let analyzer = AudioAnalyzer {
                        input: source,
                        buffer: Vec::with_capacity(2048),
                        spectrum_data: self.spectrum_data.clone(),
                        sample_rate,
                    };

                    sink.stop(); // Stop current track
                    sink.append(analyzer);
                    sink.play();
                }
            }
        } else {
            self.last_error = Some("Audio not available".to_string());
        }
    }

    pub fn toggle_pause(&mut self) {
        if let Some(sink) = &self.sink {
            if self.is_paused {
                sink.play();
                self.is_paused = false;
                self.playback_start = Some(Instant::now());
            } else {
                sink.pause();
                self.is_paused = true;
                if let Some(start) = self.playback_start {
                    self.playback_elapsed += start.elapsed();
                    self.playback_start = None;
                }
            }
        }
    }

    pub fn change_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
    }

    pub fn next_track(&mut self) {
        // Find current track index in items
        if let Some(idx) = self.current_track.as_ref().and_then(|cp| self.items.iter().position(|p| p == cp)) {
            // Find next playable file
            for i in (idx + 1)..self.items.len() {
                let path = &self.items[i];
                if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                    self.play_file(path.clone());
                    self.state.select(Some(i)); // Move selection to playing file
                    return;
                }
            }
        }
    }

    pub fn previous_track(&mut self) {
        if let Some(idx) = self.current_track.as_ref().and_then(|cp| self.items.iter().position(|p| p == cp)) {
            // Find previous playable file
            for i in (0..idx).rev() {
                let path = &self.items[i];
                if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                    self.play_file(path.clone());
                    self.state.select(Some(i));
                    return;
                }
            }
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.load_directory();
        }
    }
}

pub trait EventSource {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
    fn read(&mut self) -> io::Result<Event>;
}

pub struct CrosstermEvents;
impl EventSource for CrosstermEvents {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }
    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }
}

pub fn run_app<B: Backend, E: EventSource>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    events: &mut E,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if events.poll(Duration::from_millis(50))? {
            let event = events.read()?;
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Tab => {
                        app.mode = match app.mode {
                            AppMode::FileSystem => AppMode::Radio,
                            AppMode::Radio => AppMode::FileSystem,
                        };
                    }
                    KeyCode::Char('j') | KeyCode::Down => app.next(),
                    KeyCode::Char('k') | KeyCode::Up => app.previous(),
                    KeyCode::Char('+') | KeyCode::Char('=') => app.change_volume(0.05),
                    KeyCode::Char('-') => app.change_volume(-0.05),
                    KeyCode::Left => app.previous_track(),
                    KeyCode::Right => app.next_track(),
                    KeyCode::Char(' ') => app.toggle_pause(),
                    KeyCode::Enter => app.enter_directory(),
                    KeyCode::Backspace => app.go_up(),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        
        let names: Vec<String> = app.items.iter()
            .map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "..".to_string()))
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
        
        // Simulate playing p1
        app.current_track = Some(p1.clone());
        
        // Next track
        app.next_track();
        assert_eq!(app.current_track, Some(p2.clone()));
        
        // Next track
        app.next_track();
        assert_eq!(app.current_track, Some(p3.clone()));
        
        // Next track (should stop at end or wrap? Code says: for i in (idx + 1)..self.items.len())
        // It does NOT wrap.
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
        
        let names: Vec<String> = app.items.iter()
            .map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "..".to_string()))
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
            self.events.pop_front().ok_or(io::Error::other("No more events"))
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
        app.state.select(Some(0));

        let events = vec![
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('j'))), // Next -> 1
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('k'))), // Prev -> 0
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Tab)),       // Mode -> Radio
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Tab)),       // Mode -> FileSystem
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('+'))), // Vol Up
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('-'))), // Vol Down
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Left)),      // Prev Track
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Right)),     // Next Track
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char(' '))), // Pause
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Enter)),     // Enter Dir
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Backspace)), // Go Up
            Event::Key(crossterm::event::KeyEvent::from(KeyCode::Char('q'))), // Quit
        ];
        
        let mut event_source = MockEventSource::new(events);
        
        run_app(&mut terminal, &mut app, &mut event_source).unwrap();
        
        assert_eq!(app.state.selected(), Some(0));
        assert!(matches!(app.mode, AppMode::FileSystem));
    }
}
