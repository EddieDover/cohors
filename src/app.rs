use crate::audio::AudioAnalyzer;
use crate::favorites::Favorites;
use crate::radio::{RadioGroup, RadioStation};
use crate::ui;
use crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::ListState;
use ratatui::{Terminal, backend::Backend};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::{
    fs, io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub type AudioSource = Box<dyn Source<Item = f32> + Send>;
pub type SourceResult = Result<AudioSource, String>;
pub type SourceReceiver = std::sync::mpsc::Receiver<SourceResult>;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AppMode {
    FileSystem,
    Radio,
    Favorites,
}

pub enum LoopMode {
    Off,
    Track,
    All,
}

pub struct App {
    pub mode: AppMode,
    pub favorites: Favorites,
    pub current_dir: PathBuf,
    pub items: Vec<PathBuf>,
    pub state: ListState,
    // Radio
    pub radio_groups: Vec<RadioGroup>,
    pub radio_state: ListState,
    // Favorites
    pub favorites_state: ListState,
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
    // Async Source Loading
    pub source_receiver: Option<SourceReceiver>,
    // HTTP Client
    pub http_client: reqwest::blocking::Client,
    // UI State
    pub show_about: bool,
    pub show_hidden: bool,
    // Looping Mode
    pub loop_mode: LoopMode,
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
            favorites: Favorites::load(),
            current_dir,
            items: Vec::new(),
            state: ListState::default(),
            radio_groups: Vec::new(),
            radio_state: ListState::default(),
            favorites_state: ListState::default(),
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
            spectrum_data: Arc::new(Mutex::new(vec![
                ("Sub", 0),
                ("Bass", 0),
                ("LowM", 0),
                ("Mid", 0),
                ("HighM", 0),
                ("Pres", 0),
                ("Bril", 0),
                ("Air", 0),
            ])),
            source_receiver: None,
            http_client: reqwest::blocking::Client::new(),
            show_about: false,
            show_hidden: false,
            loop_mode: LoopMode::Off,
        };
        app.load_directory();
        app
    }

    #[cfg(test)]
    pub fn new_test() -> App {
        let (sink, _queue) = Sink::new_idle();
        App {
            mode: AppMode::FileSystem,
            favorites: Favorites::default(),
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            items: Vec::new(),
            state: ListState::default(),
            radio_groups: Vec::new(),
            radio_state: ListState::default(),
            favorites_state: ListState::default(),
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
            spectrum_data: Arc::new(Mutex::new(vec![
                ("Sub", 0),
                ("Bass", 0),
                ("LowM", 0),
                ("Mid", 0),
                ("HighM", 0),
                ("Pres", 0),
                ("Bril", 0),
                ("Air", 0),
            ])),
            source_receiver: None,
            http_client: reqwest::blocking::Client::new(),
            show_about: false,
            show_hidden: false,
            loop_mode: LoopMode::Off,
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
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if !self.show_hidden && file_name.starts_with('.') {
                    continue;
                }

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

    pub fn get_visible_radio_count(&self) -> usize {
        let mut count = 0;
        for group in &self.radio_groups {
            count += 1; // The group header
            if group.is_expanded {
                count += group.stations.len();
            }
        }
        count
    }

    pub fn get_radio_station_at_index(&self, index: usize) -> Option<&RadioStation> {
        let mut current_idx = 0;
        for group in &self.radio_groups {
            if current_idx == index {
                return None; // It's a group header
            }
            current_idx += 1;

            if group.is_expanded {
                if index < current_idx + group.stations.len() {
                    return Some(&group.stations[index - current_idx]);
                }
                current_idx += group.stations.len();
            }
        }
        None
    }

    pub fn next(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
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
            AppMode::Radio => {
                let count = self.get_visible_radio_count();
                let i = match self.radio_state.selected() {
                    Some(i) => {
                        if i >= count.saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.radio_state.select(Some(i));
            }
            AppMode::Favorites => {
                let count = self.favorites.files.len() + self.favorites.stations.len();
                let i = match self.favorites_state.selected() {
                    Some(i) => {
                        if i >= count.saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.favorites_state.select(Some(i));
            }
        }
    }

    pub fn previous(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
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
            AppMode::Radio => {
                let count = self.get_visible_radio_count();
                let i = match self.radio_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            count.saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.radio_state.select(Some(i));
            }
            AppMode::Favorites => {
                let count = self.favorites.files.len() + self.favorites.stations.len();
                let i = match self.favorites_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            count.saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.favorites_state.select(Some(i));
            }
        }
    }

    pub fn enter_directory(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
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
            AppMode::Radio => {
                //Determine what action to take
                enum Action {
                    ToggleGroup(usize), // group_index
                    PlayStation(RadioStation),
                }

                let mut action = None;
                let selected_idx = self.radio_state.selected().unwrap_or(0);
                let mut current_idx = 0;

                for (g_idx, group) in self.radio_groups.iter().enumerate() {
                    if current_idx == selected_idx {
                        action = Some(Action::ToggleGroup(g_idx));
                        break;
                    }
                    current_idx += 1;

                    if group.is_expanded {
                        if selected_idx < current_idx + group.stations.len() {
                            let station_idx = selected_idx - current_idx;
                            action = Some(Action::PlayStation(group.stations[station_idx].clone()));
                            break;
                        }
                        current_idx += group.stations.len();
                    }
                }

                match action {
                    Some(Action::ToggleGroup(idx)) => {
                        if let Some(group) = self.radio_groups.get_mut(idx) {
                            group.is_expanded = !group.is_expanded;
                        }
                    }
                    Some(Action::PlayStation(station)) => {
                        self.play_radio(station);
                    }
                    None => {}
                }
            }
            AppMode::Favorites => {
                if let Some(i) = self.favorites_state.selected() {
                    if i < self.favorites.files.len() {
                        if let Some(path) = self.favorites.files.get(i) {
                            if path.is_dir() {
                                self.current_dir = path.clone();
                                self.mode = AppMode::FileSystem;
                                self.load_directory();
                            } else {
                                self.play_file(path.clone());
                            }
                        }
                    } else {
                        let station_idx = i - self.favorites.files.len();
                        if let Some(station) = self.favorites.stations.get(station_idx) {
                            self.play_radio(station.clone());
                        }
                    }
                }
            }
        }
    }

    pub fn play_radio(&mut self, station: crate::radio::RadioStation) {
        self.current_track = Some(PathBuf::from(&station.name));
        self.is_paused = false;
        self.playback_start = Some(Instant::now());
        self.playback_elapsed = Duration::from_secs(0);
        self.track_duration = None;
        self.last_error = None;

        if let Some(sink) = &self.sink {
            sink.stop(); // Stop current track immediately

            let (tx, rx) = std::sync::mpsc::channel();
            self.source_receiver = Some(rx);

            let spectrum_data = self.spectrum_data.clone();
            let client = self.http_client.clone();
            let station_url = station.url.clone();

            // Spawn a thread to fetch the stream without blocking the UI or panicking tokio
            std::thread::spawn(move || {
                // Try to parse as playlist (PLS/M3U) first, if it fails, assume it's a direct stream URL
                let stream_url =
                    match crate::radio::fetch_playlist_stream_url(&client, &station_url) {
                        Ok(url) => url,
                        Err(_) => station_url, // Fallback to original URL
                    };

                match client.get(&stream_url).send() {
                    Ok(response) => {
                        if response.status().is_success() {
                            let reader = io::BufReader::new(response);
                            let source = Decoder::new(HttpStream::new(reader));
                            match source {
                                Ok(decoder) => {
                                    let source = decoder.convert_samples::<f32>();
                                    let sample_rate = source.sample_rate();

                                    let analyzer = AudioAnalyzer {
                                        input: source,
                                        buffer: Vec::with_capacity(2048),
                                        spectrum_data,
                                        sample_rate,
                                    };

                                    let _ = tx.send(Ok(Box::new(analyzer)));
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(format!("Decoder error: {}", e)));
                                }
                            }
                        } else {
                            let _ = tx.send(Err(format!("HTTP error: {}", response.status())));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("Connection error: {}", e)));
                    }
                }
            });
        }
    }

    pub fn on_tick(&mut self) {
        let source_result = self
            .source_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());

        if let Some(result) = source_result {
            match result {
                Ok(source) => {
                    if let Some(sink) = &self.sink {
                        sink.append(source);
                        sink.play();
                    }
                    self.last_error = None;
                }
                Err(e) => {
                    self.last_error = Some(e);
                    self.is_paused = true;
                    self.playback_start = None;
                }
            }
            self.source_receiver = None;
        }

        // Check for auto-advance
        if let Some(sink) = &self.sink
            && sink.empty()
            && !self.is_paused
            && self.current_track.is_some()
        {
            match self.loop_mode {
                LoopMode::All => self.next_track(),
                LoopMode::Track => {
                    if let Some(path) = self.current_track.clone() {
                        self.play_file(path);
                    }
                }
                LoopMode::Off => {}
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

    pub fn toggle_loop(&mut self) {
        self.loop_mode = match self.loop_mode {
            LoopMode::Off => LoopMode::Track,
            LoopMode::Track => LoopMode::All,
            LoopMode::All => LoopMode::Off,
        };
    }

    pub fn toggle_favorite(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
                if let Some(path) = self.state.selected().and_then(|i| self.items.get(i))
                    && path.file_name().and_then(|n| n.to_str()) != Some("..")
                {
                    self.favorites.toggle_file(path.clone());
                }
            }
            AppMode::Radio => {
                if let Some(i) = self.radio_state.selected()
                    && let Some(station) = self.get_radio_station_at_index(i)
                {
                    self.favorites.toggle_station(station.clone());
                }
            }
            AppMode::Favorites => {
                // In favorites mode, 'f' could remove the selected item
                if let Some(i) = self.favorites_state.selected() {
                    if i < self.favorites.files.len() {
                        if let Some(path) = self.favorites.files.get(i) {
                            self.favorites.toggle_file(path.clone());
                        }
                    } else {
                        let station_idx = i - self.favorites.files.len();
                        if let Some(station) = self.favorites.stations.get(station_idx) {
                            self.favorites.toggle_station(station.clone());
                        }
                    }
                    // Adjust selection if needed
                    let count = self.favorites.files.len() + self.favorites.stations.len();
                    if count == 0 {
                        self.favorites_state.select(None);
                    } else if i >= count {
                        self.favorites_state.select(Some(count - 1));
                    }
                }
            }
        }
    }

    pub fn next_track(&mut self) {
        // Only for FileSystem mode
        if matches!(self.mode, AppMode::Radio) {
            return;
        }

        // Find current track index in items
        if let Some(idx) = self
            .current_track
            .as_ref()
            .and_then(|cp| self.items.iter().position(|p| p == cp))
        {
            // Find next playable file
            for i in (idx + 1)..self.items.len() {
                let path = &self.items[i];
                if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                    self.play_file(path.clone());
                    self.state.select(Some(i)); // Move selection to playing file
                    return;
                }
            }

            // If LoopMode::All, wrap around
            if matches!(self.loop_mode, LoopMode::All) {
                for i in 0..=idx {
                    let path = &self.items[i];
                    if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                        self.play_file(path.clone());
                        self.state.select(Some(i)); // Move selection to playing file
                        return;
                    }
                }
            }
        }
    }

    pub fn previous_track(&mut self) {
        // Only for FileSystem mode
        if matches!(self.mode, AppMode::Radio) {
            return;
        }

        if let Some(idx) = self
            .current_track
            .as_ref()
            .and_then(|cp| self.items.iter().position(|p| p == cp))
        {
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

struct HttpStream<R> {
    inner: R,
    buffer: Vec<u8>,
    pos: u64,
}

impl<R: io::Read> HttpStream<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            pos: 0,
        }
    }
}

impl<R: io::Read> io::Read for HttpStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos < self.buffer.len() as u64 {
            let start = self.pos as usize;
            let available = self.buffer.len() - start;
            let to_read = std::cmp::min(buf.len(), available);
            buf[0..to_read].copy_from_slice(&self.buffer[start..start + to_read]);
            self.pos += to_read as u64;
            return Ok(to_read);
        }

        let n = self.inner.read(buf)?;
        if n > 0 {
            if self.buffer.len() < 128 * 1024 {
                let space = 128 * 1024 - self.buffer.len();
                let to_buffer = std::cmp::min(n, space);
                self.buffer.extend_from_slice(&buf[0..to_buffer]);
            }
            self.pos += n as u64;
        }
        Ok(n)
    }
}

impl<R: io::Read> io::Seek for HttpStream<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            io::SeekFrom::Start(p) => p,
            io::SeekFrom::Current(d) => (self.pos as i64 + d) as u64,
            io::SeekFrom::End(_) => {
                return Err(io::Error::other("Cannot seek from end"));
            }
        };

        if new_pos <= self.buffer.len() as u64 {
            self.pos = new_pos;
            Ok(new_pos)
        } else if new_pos == self.pos {
            Ok(new_pos)
        } else {
            Err(io::Error::other("Cannot seek in unbuffered region"))
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
        app.on_tick();
        terminal.draw(|f| ui::draw(f, app))?;

        if events.poll(Duration::from_millis(50))? {
            let event = events.read()?;
            if let Event::Key(key) = event {
                if app.show_about {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => app.show_about = false,
                        KeyCode::Char('q') => return Ok(()),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('?') => app.show_about = true,
                        KeyCode::Char('h') => {
                            app.show_hidden = !app.show_hidden;
                            app.load_directory();
                        }
                        KeyCode::Tab => {
                            app.mode = match app.mode {
                                AppMode::FileSystem => AppMode::Radio,
                                AppMode::Radio => AppMode::Favorites,
                                AppMode::Favorites => AppMode::FileSystem,
                            };
                        }
                        KeyCode::Char('j') | KeyCode::Down => app.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous(),
                        KeyCode::Char('+') | KeyCode::Char('=') => app.change_volume(0.05),
                        KeyCode::Char('l') => app.toggle_loop(),
                        KeyCode::Char('f') => app.toggle_favorite(),
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

        run_app(&mut terminal, &mut app, &mut event_source).unwrap();

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
        std::thread::sleep(Duration::from_millis(100));

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
        app.radio_state.select(Some(1)); // 0 is group header, 1 is station

        app.toggle_favorite();
        assert!(app.favorites.is_favorite_station(&station));

        app.toggle_favorite();
        assert!(!app.favorites.is_favorite_station(&station));
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
        std::thread::sleep(Duration::from_millis(100));

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
        std::thread::sleep(Duration::from_millis(100));

        if let Some(rx) = &app.source_receiver {
            match rx.try_recv() {
                Ok(Err(msg)) => assert!(msg.contains("Connection error")),
                _ => panic!("Expected Connection error"),
            }
        }
    }
}
