use crate::audio::AudioAnalyzer;
use crate::favorites::Favorites;
use crate::mpris::MprisCommand;
use crate::radio::{RadioGroup, RadioStation};
use crate::ui;
use crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::ListState;
use ratatui::{Terminal, backend::Backend};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::{
    fs, io,
    path::PathBuf,
    sync::mpsc::Receiver,
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
    pub show_help: bool,
    pub show_hidden: bool,
    // Looping Mode
    pub loop_mode: LoopMode,
    // Search
    pub is_searching: bool,
    pub search_query: String,
    pub filtered_items: Vec<PathBuf>,
    pub filtered_radio_groups: Vec<RadioGroup>,
    pub mpris_state: Option<std::sync::Arc<std::sync::Mutex<crate::mpris::MprisState>>>,
    pub mpris_notifier: Option<tokio::sync::mpsc::UnboundedSender<()>>,
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
            show_help: false,
            show_hidden: false,
            loop_mode: LoopMode::Off,
            is_searching: false,
            search_query: String::new(),
            filtered_items: Vec::new(),
            filtered_radio_groups: Vec::new(),
            mpris_state: None,
            mpris_notifier: None,
        };
        app.load_directory();
        app
    }

    #[cfg(test)]
    pub fn new_test() -> App {
        let (sink, queue) = Sink::new_idle();
        std::mem::forget(queue);
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
            show_help: false,
            show_hidden: false,
            loop_mode: LoopMode::Off,
            is_searching: false,
            search_query: String::new(),
            filtered_items: Vec::new(),
            filtered_radio_groups: Vec::new(),
            mpris_state: None,
            mpris_notifier: None,
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
        self.update_search_results();
        self.state.select(Some(0));
    }

    pub fn update_search_results(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_items = self.items.clone();
            self.filtered_radio_groups = self.radio_groups.clone();
        } else {
            let query = self.search_query.to_lowercase();
            // Filter files
            self.filtered_items = self
                .items
                .iter()
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_lowercase().contains(&query))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();

            // Filter radio
            self.filtered_radio_groups = self
                .radio_groups
                .iter()
                .map(|g| {
                    let mut new_group = g.clone();
                    new_group
                        .stations
                        .retain(|s| s.name.to_lowercase().contains(&query));
                    new_group
                })
                .filter(|g| !g.stations.is_empty())
                .collect();
        }
    }

    pub fn on_search_input(&mut self, c: char) {
        self.search_query.push(c);
        self.update_search_results();
        // Reset selection
        match self.mode {
            AppMode::FileSystem => self.state.select(Some(0)),
            AppMode::Radio => self.radio_state.select(Some(0)),
            AppMode::Favorites => self.favorites_state.select(Some(0)),
        }
    }

    pub fn on_search_backspace(&mut self) {
        self.search_query.pop();
        self.update_search_results();
        // Reset selection
        match self.mode {
            AppMode::FileSystem => self.state.select(Some(0)),
            AppMode::Radio => self.radio_state.select(Some(0)),
            AppMode::Favorites => self.favorites_state.select(Some(0)),
        }
    }

    pub fn cancel_search(&mut self) {
        self.is_searching = false;
        self.search_query.clear();
        self.update_search_results();
        // Reset selection
        match self.mode {
            AppMode::FileSystem => self.state.select(Some(0)),
            AppMode::Radio => self.radio_state.select(Some(0)),
            AppMode::Favorites => self.favorites_state.select(Some(0)),
        }
    }

    pub fn submit_search(&mut self) {
        self.is_searching = false;
        // Keep the filter active
    }

    pub fn get_visible_radio_count(&self) -> usize {
        let mut count = 0;
        for group in &self.filtered_radio_groups {
            count += 1; // The group header
            if group.is_expanded {
                count += group.stations.len();
            }
        }
        count
    }

    pub fn get_radio_station_at_index(&self, index: usize) -> Option<&RadioStation> {
        let mut current_idx = 0;
        for group in &self.filtered_radio_groups {
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
                        if i >= self.filtered_items.len().saturating_sub(1) {
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
                            self.filtered_items.len().saturating_sub(1)
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
                if let Some(path) = self
                    .state
                    .selected()
                    .and_then(|i| self.filtered_items.get(i))
                {
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

                for (g_idx, group) in self.filtered_radio_groups.iter().enumerate() {
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
                        // We need to find the corresponding group in the original list to toggle it
                        // Because filtered_radio_groups is derived
                        if let Some(filtered_group) = self.filtered_radio_groups.get(idx) {
                            let title = &filtered_group.title;
                            if let Some(original_group) =
                                self.radio_groups.iter_mut().find(|g| &g.title == title)
                            {
                                original_group.is_expanded = !original_group.is_expanded;
                            }
                        }
                        // Re-filter to update view
                        self.update_search_results();
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

    pub fn seek_to(&mut self, position: Duration) {
        if let Some(sink) = &self.sink {
            match sink.try_seek(position) {
                Ok(_) => {
                    self.playback_elapsed = position;
                    if !self.is_paused {
                        self.playback_start = Some(Instant::now());
                    }
                    self.update_mpris();
                }
                Err(e) => {
                    // Fallback for sources that don't support seeking (like some FLAC decoders)
                    if !self.seek_fallback(position) {
                        self.last_error = Some(format!("Seek error: {}", e));
                    }
                }
            }
        }
    }

    fn seek_fallback(&mut self, position: Duration) -> bool {
        if let Some(path) = &self.current_track
            && path.exists()
            && let Ok(file) = fs::File::open(path)
        {
            let reader = io::BufReader::new(file);
            if let Ok(decoder) = Decoder::new(reader) {
                let source = decoder.convert_samples::<f32>();
                let sample_rate = source.sample_rate();

                // Skip to position
                let source = source.skip_duration(position);

                let analyzer = AudioAnalyzer {
                    input: source,
                    buffer: Vec::with_capacity(2048),
                    spectrum_data: self.spectrum_data.clone(),
                    sample_rate,
                };

                if let Some(sink) = &self.sink {
                    sink.stop();
                    sink.append(analyzer);

                    if self.is_paused {
                        sink.pause();
                        self.playback_start = None;
                    } else {
                        sink.play();
                        self.playback_start = Some(Instant::now());
                    }
                }

                self.playback_elapsed = position;
                self.update_mpris();
                self.last_error = None;
                return true;
            }
        }
        false
    }

    pub fn handle_mpris_command(&mut self, cmd: MprisCommand) {
        match cmd {
            MprisCommand::Play => {
                if self.is_paused {
                    self.toggle_pause();
                }
            }
            MprisCommand::Pause => {
                if !self.is_paused {
                    self.toggle_pause();
                }
            }
            MprisCommand::PlayPause => self.toggle_pause(),
            MprisCommand::Stop => {
                if !self.is_paused {
                    self.toggle_pause();
                }
            }
            MprisCommand::Next => self.next_track(),
            MprisCommand::Previous => self.previous_track(),
            MprisCommand::Volume(vol) => {
                self.volume = vol as f32;
                if let Some(sink) = &self.sink {
                    sink.set_volume(self.volume);
                }
                self.update_mpris();
            }
            MprisCommand::Seek(offset_micros) => {
                let current_pos = self.playback_elapsed
                    + self
                        .playback_start
                        .map(|s| s.elapsed())
                        .unwrap_or(Duration::from_secs(0));

                let new_pos_micros = current_pos.as_micros() as i128 + offset_micros as i128;
                let new_pos = if new_pos_micros < 0 {
                    Duration::from_secs(0)
                } else {
                    let pos = Duration::from_micros(new_pos_micros as u64);
                    if let Some(duration) = self.track_duration {
                        if pos > duration { duration } else { pos }
                    } else {
                        pos
                    }
                };
                self.seek_to(new_pos);
            }
            MprisCommand::SetPosition(_track_id, position_micros) => {
                let mut pos = Duration::from_micros(position_micros as u64);
                if let Some(duration) = self.track_duration
                    && pos > duration
                {
                    pos = duration;
                }
                self.seek_to(pos);
            }
            MprisCommand::LoopStatus(status) => {
                self.loop_mode = match status {
                    mpris_server::LoopStatus::None => LoopMode::Off,
                    mpris_server::LoopStatus::Track => LoopMode::Track,
                    mpris_server::LoopStatus::Playlist => LoopMode::All,
                };
                self.update_mpris();
            }
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
        self.update_mpris();
    }

    pub fn update_mpris(&self) {
        if let Some(mpris_state) = &self.mpris_state {
            if let Ok(mut state) = mpris_state.lock() {
                state.playback_status = if self.current_track.is_some() {
                    if let Some(sink) = &self.sink {
                        if sink.empty() {
                            mpris_server::PlaybackStatus::Stopped
                        } else if self.is_paused {
                            mpris_server::PlaybackStatus::Paused
                        } else {
                            mpris_server::PlaybackStatus::Playing
                        }
                    } else {
                        mpris_server::PlaybackStatus::Stopped
                    }
                } else {
                    mpris_server::PlaybackStatus::Stopped
                };

                state.volume = self.volume as f64;
                state.loop_status = match self.loop_mode {
                    LoopMode::Off => mpris_server::LoopStatus::None,
                    LoopMode::Track => mpris_server::LoopStatus::Track,
                    LoopMode::All => mpris_server::LoopStatus::Playlist,
                };
                state.position = self.playback_elapsed;
                state.playback_start = if self.is_paused {
                    None
                } else {
                    self.playback_start
                };

                if let Some(path) = &self.current_track {
                    let title = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    state.title = title;
                    state.duration = self.track_duration;
                } else {
                    state.title = String::new();
                    state.duration = None;
                }
            }
            if let Some(notifier) = &self.mpris_notifier {
                notifier.send(()).ok();
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
        self.update_mpris();
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
        self.update_mpris();
    }

    pub fn change_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
        self.update_mpris();
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

        // Find current track index in filtered items
        if let Some(idx) = self
            .current_track
            .as_ref()
            .and_then(|cp| self.filtered_items.iter().position(|p| p == cp))
        {
            // Find next playable file
            for i in (idx + 1)..self.filtered_items.len() {
                let path = &self.filtered_items[i];
                if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                    self.play_file(path.clone());
                    self.state.select(Some(i)); // Move selection to playing file
                    return;
                }
            }

            // If LoopMode::All, wrap around
            if matches!(self.loop_mode, LoopMode::All) {
                for i in 0..=idx {
                    let path = &self.filtered_items[i];
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
            .and_then(|cp| self.filtered_items.iter().position(|p| p == cp))
        {
            // Find previous playable file
            for i in (0..idx).rev() {
                let path = &self.filtered_items[i];
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
    mpris_rx: &Receiver<MprisCommand>,
) -> io::Result<()> {
    loop {
        app.on_tick();

        // Handle MPRIS commands
        while let Ok(cmd) = mpris_rx.try_recv() {
            app.handle_mpris_command(cmd);
        }

        terminal.draw(|f| ui::draw(f, app))?;

        if events.poll(Duration::from_millis(50))? {
            let event = events.read()?;
            if let Event::Key(key) = event {
                if app.is_searching {
                    match key.code {
                        KeyCode::Esc => app.cancel_search(),
                        KeyCode::Enter => app.submit_search(),
                        KeyCode::Backspace => app.on_search_backspace(),
                        KeyCode::Char(c) => app.on_search_input(c),
                        _ => {}
                    }
                } else if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => app.show_help = false,
                        KeyCode::Char('q') => return Ok(()),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('?') => app.show_help = true,
                        KeyCode::Char('/') => {
                            app.is_searching = true;
                            app.search_query.clear();
                            app.update_search_results();
                        }
                        KeyCode::Esc => {
                            if !app.search_query.is_empty() {
                                app.cancel_search();
                            }
                        }
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
                            // Reset search when switching modes?
                            // Maybe better to keep it separate or clear it.
                            // Let's clear it for simplicity.
                            app.cancel_search();
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
mod tests;
#[cfg(test)]
mod tests_mpris_logic;
